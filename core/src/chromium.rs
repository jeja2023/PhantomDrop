use std::env;

const SANDBOX_ENV: &str = "PHANTOM_CHROME_SANDBOX";

pub(crate) fn sandbox_enabled() -> bool {
    env::var(SANDBOX_ENV)
        .map(|value| sandbox_enabled_from_value(&value))
        .unwrap_or(true)
}

fn sandbox_enabled_from_value(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
    )
}

#[cfg(test)]
mod tests {
    use super::sandbox_enabled_from_value;

    #[test]
    fn disables_sandbox_for_common_false_values() {
        for value in ["0", "false", "FALSE", " no ", "Off"] {
            assert!(!sandbox_enabled_from_value(value), "value={value}");
        }
    }

    #[test]
    fn keeps_sandbox_for_true_or_unknown_values() {
        for value in ["1", "true", "yes", "on", "unexpected", ""] {
            assert!(sandbox_enabled_from_value(value), "value={value}");
        }
    }
}
