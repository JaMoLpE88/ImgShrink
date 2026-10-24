use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Sender};

use crate::engine::{self, EngineStage};
use crate::error::CoreError;
use crate::format::ImageFormat;
use crate::journal::Journal;
use crate::options::OptimizeOptions;
use crate::output::{write_item, Destination};
use crate::plan::{ConflictPolicy, OperationPlan, SkipReason};
use crate::stats::Summary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Decode,
    Transform,
    Encode,
    Verify,
    Write,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Decode => "Decodificando",
            Stage::Transform => "Transformando",
            Stage::Encode => "Codificando",
            Stage::Verify => "Verificando",
            Stage::Write => "Guardando",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ItemResult {
    Done {
        output: PathBuf,
        output_bytes: u64,
        duration: Duration,
        width: u32,
        height: u32,
    },
    Skipped {
        reason: SkipReason,
    },
    Failed {
        code: String,
        message: String,
    },
    Cancelled,
}

#[derive(Clone, Debug)]
pub enum JobEvent {
    Stage {
        id: usize,
        stage: Stage,
    },
    ItemFinished {
        id: usize,
        result: ItemResult,
    },
    Totals {
        summary: Summary,
    },
    Finished {
        summary: Summary,
        results: Vec<(usize, ItemResult)>,
    },
}

#[derive(Clone, Debug)]
pub struct JobOptions {
    pub workers: usize,
    pub conflict_policy: ConflictPolicy,
    pub journal: Option<Arc<Journal>>,
}

#[derive(Default)]
struct Counters {
    succeeded: usize,
    skipped: usize,
    failed: usize,
    cancelled: usize,
    input_bytes: u64,
    output_bytes: u64,
}

pub fn run(
    plan: &OperationPlan,
    options: &OptimizeOptions,
    job: &JobOptions,
    tx: Sender<JobEvent>,
    cancel: Arc<AtomicBool>,
) -> Summary {
    let started = Instant::now();
    let workers = job.workers.max(1);
    let in_place = matches!(plan.destination, Destination::InPlace);
    let counters = Arc::new(Mutex::new(Counters::default()));
    let results = Arc::new(Mutex::new(Vec::<(usize, ItemResult)>::new()));

    for item in &plan.items {
        if let Some(reason) = item.skip {
            let mut counter = counters.lock().unwrap();
            counter.skipped += 1;
            drop(counter);
            results
                .lock()
                .unwrap()
                .push((item.source.id, ItemResult::Skipped { reason }));
        }
    }

    let pending: Vec<usize> = plan
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.skip.is_none())
        .map(|(index, _)| index)
        .collect();

    if !pending.is_empty() {
        let (task_tx, task_rx) = bounded::<usize>(pending.len().max(1));
        for index in &pending {
            let _ = task_tx.send(*index);
        }
        drop(task_tx);

        std::thread::scope(|scope| {
            for _ in 0..workers.min(pending.len()) {
                let task_rx = task_rx.clone();
                let counters = Arc::clone(&counters);
                let results = Arc::clone(&results);
                let tx = tx.clone();
                let cancel = Arc::clone(&cancel);
                scope.spawn(move || {
                    while let Ok(index) = task_rx.recv() {
                        if cancel.load(Ordering::Relaxed) {
                            record(
                                plan.items[index].source.id,
                                ItemResult::Cancelled,
                                &plan.items[index].source.size,
                                &counters,
                                &results,
                                &tx,
                                plan.items.len(),
                                started,
                            );
                            continue;
                        }
                        let item_started = Instant::now();
                        let result =
                            process_item(&plan.items[index], in_place, options, job, &tx, &cancel);
                        let result = match result {
                            Ok(Some((output, output_bytes, width, height))) => ItemResult::Done {
                                output,
                                output_bytes,
                                duration: item_started.elapsed(),
                                width,
                                height,
                            },
                            Ok(None) => ItemResult::Skipped {
                                reason: SkipReason::NoImprovement,
                            },
                            Err(CoreError::Cancelled) => ItemResult::Cancelled,
                            Err(error) => ItemResult::Failed {
                                code: error.code().to_string(),
                                message: error.to_string(),
                            },
                        };
                        record(
                            plan.items[index].source.id,
                            result,
                            &plan.items[index].source.size,
                            &counters,
                            &results,
                            &tx,
                            plan.items.len(),
                            started,
                        );
                    }
                });
            }
        });
    }

    let mut all_results = results.lock().unwrap().clone();
    for (index, item) in plan.items.iter().enumerate() {
        if item.skip.is_none() && !all_results.iter().any(|(id, _)| *id == item.source.id) {
            all_results.push((item.source.id, ItemResult::Cancelled));
            counters.lock().unwrap().cancelled += 1;
            let _ = index;
        }
    }

    let counter = counters.lock().unwrap();
    let summary = Summary {
        found: plan.items.len(),
        succeeded: counter.succeeded,
        skipped: counter.skipped,
        failed: counter.failed,
        cancelled: counter.cancelled,
        input_bytes: counter.input_bytes,
        output_bytes: counter.output_bytes,
        elapsed_secs: started.elapsed().as_secs_f64(),
    };
    drop(counter);
    all_results.sort_by_key(|(id, _)| *id);
    let _ = tx.send(JobEvent::Finished {
        summary: summary.clone(),
        results: all_results,
    });
    summary
}

#[allow(clippy::too_many_arguments)]
fn record(
    index: usize,
    result: ItemResult,
    input_size: &u64,
    counters: &Arc<Mutex<Counters>>,
    results: &Arc<Mutex<Vec<(usize, ItemResult)>>>,
    tx: &Sender<JobEvent>,
    found: usize,
    started: Instant,
) {
    {
        let mut counter = counters.lock().unwrap();
        match &result {
            ItemResult::Done { output_bytes, .. } => {
                counter.succeeded += 1;
                counter.input_bytes = counter.input_bytes.saturating_add(*input_size);
                counter.output_bytes = counter.output_bytes.saturating_add(*output_bytes);
            }
            ItemResult::Skipped { .. } => counter.skipped += 1,
            ItemResult::Failed { .. } => counter.failed += 1,
            ItemResult::Cancelled => counter.cancelled += 1,
        }
        let summary = Summary {
            found,
            succeeded: counter.succeeded,
            skipped: counter.skipped,
            failed: counter.failed,
            cancelled: counter.cancelled,
            input_bytes: counter.input_bytes,
            output_bytes: counter.output_bytes,
            elapsed_secs: started.elapsed().as_secs_f64(),
        };
        drop(counter);
        let _ = tx.send(JobEvent::Totals { summary });
    }
    results.lock().unwrap().push((index, result.clone()));
    let _ = tx.send(JobEvent::ItemFinished { id: index, result });
}

fn process_item(
    plan_item: &crate::plan::PlanItem,
    in_place: bool,
    options: &OptimizeOptions,
    job: &JobOptions,
    tx: &Sender<JobEvent>,
    cancel: &Arc<AtomicBool>,
) -> Result<Option<(PathBuf, u64, u32, u32)>, CoreError> {
    let item = &plan_item.source;
    let stage_tx = tx.clone();
    let id = item.id;
    let optimized = engine::optimize_with(item, options, move |stage| {
        let stage = match stage {
            EngineStage::Decode => Stage::Decode,
            EngineStage::Transform => Stage::Transform,
            EngineStage::Encode => Stage::Encode,
        };
        let _ = stage_tx.try_send(JobEvent::Stage { id, stage });
    })?;

    if cancel.load(Ordering::Relaxed) {
        return Err(CoreError::Cancelled);
    }
    if options.keep_if_larger && optimized.bytes.len() as u64 >= item.size {
        return Ok(None);
    }

    let _ = tx.try_send(JobEvent::Stage {
        id,
        stage: Stage::Verify,
    });
    let _ = tx.try_send(JobEvent::Stage {
        id,
        stage: Stage::Write,
    });
    let output_path = plan_item.output.clone();
    let output = write_item(
        item,
        &output_path,
        &optimized.bytes,
        optimized.format,
        in_place,
        job.conflict_policy,
        job.journal.as_deref(),
    )?;
    Ok(Some((
        output,
        optimized.bytes.len() as u64,
        optimized.width,
        optimized.height,
    )))
}

/// Variante útil para pruebas y para la CLI sin TUI.
pub fn run_blocking(
    plan: &OperationPlan,
    options: &OptimizeOptions,
    job: &JobOptions,
) -> (Summary, Vec<(usize, ItemResult)>) {
    let (tx, rx) = bounded(1024);
    let cancel = Arc::new(AtomicBool::new(false));
    let plan = plan.clone();
    let options = options.clone();
    let job = job.clone();
    let handle = std::thread::spawn(move || run(&plan, &options, &job, tx, cancel));
    let mut results = Vec::new();
    let mut summary = Summary::default();
    for event in rx {
        if let JobEvent::Finished {
            summary: finished,
            results: finished_results,
        } = event
        {
            summary = finished;
            results = finished_results;
            break;
        }
    }
    let _ = handle.join();
    (summary, results)
}

pub fn format_for(item: &crate::discovery::SourceItem, options: &OptimizeOptions) -> ImageFormat {
    options.effective_format(item.format)
}
