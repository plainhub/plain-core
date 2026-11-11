use super::*;
#[derive(Debug)]
struct Root;
impl std::fmt::Display for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("root failure")
    }
}
impl std::error::Error for Root {}
#[test]
fn contexts_preserve_error_identity_chain_and_display() {
    let error = Err::<(), _>(Root)
        .context("inner")
        .context("outer")
        .unwrap_err();
    assert!(error.downcast_ref::<Root>().is_some());
    assert_eq!(
        error.chain().map(ToString::to_string).collect::<Vec<_>>(),
        ["outer", "inner", "root failure"]
    );
    assert_eq!(format!("{error:#}"), "outer: inner: root failure");
    assert_eq!(
        error.downcast::<Root>().unwrap().to_string(),
        "root failure"
    );
}
#[test]
fn macros_preserve_early_returns_and_evaluate_conditions_once() {
    let mut calls = 0;
    let value = (|| -> Result<()> {
        crate::anyhow::ensure!(
            {
                calls += 1;
                false
            },
            "bad {}",
            7
        );
        Ok(())
    })();
    assert_eq!(calls, 1);
    assert_eq!(value.unwrap_err().to_string(), "bad 7");
    let value = (|| -> Result<()> {
        crate::anyhow::bail!("stop");
    })();
    assert_eq!(value.unwrap_err().to_string(), "stop");
    assert_eq!(crate::anyhow::anyhow!(Root).to_string(), "root failure");
}

#[test]
fn successful_ensure_does_not_evaluate_its_error_argument() {
    let mut errors = 0;
    let result = (|| -> Result<()> {
        crate::anyhow::ensure!(true, {
            errors += 1;
            "not evaluated"
        });
        Ok(())
    })();
    result.unwrap();
    assert_eq!(errors, 0);
}
