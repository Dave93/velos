use velos_core::VelosError;

pub async fn run(name: String, count_str: String, json: bool) -> Result<(), VelosError> {
    let mut client = super::connect().await?;

    // Resolve target count: absolute (4), relative (+2, -1), or "max"
    let target_count = resolve_target_count(&mut client, &name, &count_str).await?;

    let result = client.scale(&name, target_count).await?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "name": name,
                "target": target_count,
                "started": result.started,
                "stopped": result.stopped,
            })
        );
    } else if result.started > 0 {
        println!(
            "[velos] Scaled '{}' up: +{} instances (target={})",
            name, result.started, target_count
        );
    } else if result.stopped > 0 {
        println!(
            "[velos] Scaled '{}' down: -{} instances (target={})",
            name, result.stopped, target_count
        );
    } else {
        println!("[velos] '{name}' already at {target_count} instances");
    }

    Ok(())
}

async fn resolve_target_count(
    client: &mut velos_client::VelosClient,
    name: &str,
    count_str: &str,
) -> Result<u32, VelosError> {
    let s = count_str.trim();

    // "max" = CPU cores
    if s.eq_ignore_ascii_case("max") {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1);
        return Ok(cpus);
    }

    // Relative: +N or -N
    if s.starts_with('+') || s.starts_with('-') {
        let delta: i32 = s
            .parse()
            .map_err(|_| VelosError::ProtocolError(format!("invalid count: '{s}'")))?;

        // Get current instance count
        let procs = client.list().await?;
        let current = procs
            .iter()
            .filter(|p| {
                p.name == name
                    || (p.name.len() > name.len()
                        && p.name.starts_with(name)
                        && p.name.as_bytes().get(name.len()) == Some(&b':')
                        && p.name[name.len() + 1..].parse::<u32>().is_ok())
            })
            .count() as i32;

        let target = (current + delta).max(0) as u32;
        return Ok(target);
    }

    // Absolute number
    s.parse::<u32>()
        .map_err(|_| VelosError::ProtocolError(format!("invalid count: '{s}'")))
}
