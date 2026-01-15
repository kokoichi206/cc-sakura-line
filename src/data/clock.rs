pub fn now_hms() -> String {
    let output = std::process::Command::new("date").arg("+%H:%M:%S").output();

    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "-".to_string(),
    }
}
