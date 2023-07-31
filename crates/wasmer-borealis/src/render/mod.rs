use std::io::Write;

use anyhow::Error;

use crate::experiment::Results;

#[tracing::instrument(skip_all)]
pub fn html(report: &Results) -> Result<String, Error> {
    Ok(String::new())
}

pub fn text(report: &Results, dest: &mut dyn Write) -> Result<(), Error> {
    let Results {
        reports,
        total_time,
        experiment_dir,
    } = report;
    let mut success = 0;
    let mut failures = 0;
    let mut bugs = 0;

    for report in reports {
        match &report.outcome {
            crate::experiment::Outcome::Completed { status, .. } if status.success() => {
                success += 1
            }
            crate::experiment::Outcome::Completed { .. } => failures += 1,
            crate::experiment::Outcome::FetchFailed { .. }
            | crate::experiment::Outcome::SetupFailed { .. }
            | crate::experiment::Outcome::SpawnFailed { .. } => bugs += 1,
        }
    }

    writeln!(dest, "Experiment result... success: {success}, failures: {failures}, bugs: {bugs}. Finished in {total_time:?}")?;
    writeln!(dest, "Experiment dir: {}", experiment_dir.display())?;

    Ok(())
}
