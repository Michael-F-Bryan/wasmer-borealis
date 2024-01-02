use std::io::Write;

use anyhow::Error;
use once_cell::sync::Lazy;

use crate::experiment::{Report, Results};

static TEMPLATES: Lazy<minijinja::Environment<'static>> = Lazy::new(|| {
    let mut env = minijinja::Environment::new();
    env.add_template("report", include_str!("report.html.jinja"))
        .unwrap();
    env.add_filter("file_url", file_url);
    env
});

fn file_url(path: String) -> String {
    url::Url::from_file_path(&path)
        .map(|u| u.to_string())
        .unwrap_or(path)
}

#[tracing::instrument(skip_all)]
pub fn html(results: &Results) -> Result<String, Error> {
    let Results {
        experiment,
        reports,
        total_time,
        experiment_dir,
    } = results;

    let ctx = minijinja::context! {
        experiment,
        reports => ReportCategories::new(reports),
        total_time => format!("{total_time:.1?}"),
        experiment_dir,
    };

    let rendered = TEMPLATES.get_template("report")?.render(ctx)?;
    Ok(rendered)
}

#[derive(Debug, serde::Serialize)]
struct ReportCategories<'a> {
    bugs: Vec<&'a Report>,
    success: Vec<&'a Report>,
    failures: Vec<&'a Report>,
    all: Vec<&'a Report>,
    total: usize,
}

impl<'a> ReportCategories<'a> {
    fn new(reports: &'a [Report]) -> Self {
        let mut bugs = Vec::new();
        let mut success = Vec::new();
        let mut failures = Vec::new();

        for report in reports {
            match &report.outcome {
                crate::experiment::Outcome::Completed { status, .. } if status.success => {
                    success.push(report);
                }
                crate::experiment::Outcome::Completed { .. } => failures.push(report),
                crate::experiment::Outcome::FetchFailed { .. }
                | crate::experiment::Outcome::SetupFailed { .. }
                | crate::experiment::Outcome::SpawnFailed { .. } => bugs.push(report),
            }
        }

        let sort = |items: &mut [&Report]| {
            items.sort_by_key(|r| {
                (
                    r.display_name.as_str(),
                    std::cmp::Reverse(r.package_version.version.as_str()),
                )
            });
        };

        let mut all: Vec<&Report> = reports.iter().collect();
        sort(&mut bugs);
        sort(&mut success);
        sort(&mut failures);
        sort(&mut all);

        ReportCategories {
            bugs,
            success,
            failures,
            all,
            total: reports.len(),
        }
    }
}

pub fn text(results: &Results, mut dest: impl Write) -> Result<(), Error> {
    let Results {
        experiment: _,
        reports,
        total_time,
        ..
    } = results;

    let mut success = 0;
    let mut failures = 0;
    let mut bugs = 0;

    for report in reports {
        match &report.outcome {
            crate::experiment::Outcome::Completed { status, .. } if status.success => success += 1,
            crate::experiment::Outcome::Completed { .. } => failures += 1,
            crate::experiment::Outcome::FetchFailed { .. }
            | crate::experiment::Outcome::SetupFailed { .. }
            | crate::experiment::Outcome::SpawnFailed { .. } => bugs += 1,
        }
    }

    writeln!(dest, "Experiment result... success: {success}, failures: {failures}, bugs: {bugs}. Finished in {total_time:?}")?;

    Ok(())
}
