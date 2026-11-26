//! Display captions for built-in XY series. Model names and recorded units are opaque.
use serde_json::Value;

fn quantity(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "kinetic" => ("Kinetic energy", "动能"),
        "internal" => ("Internal energy", "内能"),
        "hourglass" => ("Hourglass energy", "沙漏能"),
        "viscous" => ("Viscous energy", "黏性能"),
        "external" => ("External energy", "外力功"),
        "constraint" => ("Constraint energy", "约束功"),
        "contact" => ("Contact energy", "接触能"),
        "friction" => ("Friction energy", "摩擦耗能"),
        "plastic" => ("Plastic energy", "塑性耗能"),
        "eroded" => ("Eroded energy", "侵蚀能"),
        "added_kinetic" => ("Added kinetic energy", "附加动能"),
        "balance" => ("Balance energy", "能量平衡"),
        "constraint_force_x" => ("Constraint force X", "X 向约束力"),
        "constraint_force_y" => ("Constraint force Y", "Y 向约束力"),
        "constraint_force_z" => ("Constraint force Z", "Z 向约束力"),
        "momentum_x" => ("Momentum X", "X 向动量"),
        "momentum_y" => ("Momentum Y", "Y 向动量"),
        "momentum_z" => ("Momentum Z", "Z 向动量"),
        "displacement_x" => ("Displacement X", "X 向位移"),
        "displacement_y" => ("Displacement Y", "Y 向位移"),
        "displacement_z" => ("Displacement Z", "Z 向位移"),
        "displacement" => ("Displacement magnitude", "位移幅值"),
        "velocity_x" => ("Velocity X", "X 向速度"),
        "velocity_y" => ("Velocity Y", "Y 向速度"),
        "velocity_z" => ("Velocity Z", "Z 向速度"),
        "velocity" => ("Velocity magnitude", "速度幅值"),
        "increment" => ("Time increment", "时间增量"),
        "frequency" => ("Frequency", "频率"),
        "load_factor" => ("Buckling load factor", "屈曲载荷因子"),
        _ => return None,
    })
}

pub fn picker(id: &str) -> String {
    if solidcraft_ui_text::language() == "zh"
        && let Some((_, label)) = quantity(id)
    {
        return label.into();
    }
    id.replace('_', " ")
}

fn with_unit(raw: &str, en: &str, zh: &str) -> Option<String> {
    let suffix = raw.strip_prefix(en)?;
    (suffix.is_empty() || (suffix.starts_with(" [") && suffix.ends_with(']'))).then(|| format!("{zh}{suffix}"))
}

pub fn axis_caption(axis: &str, x: &Value, raw: &str) -> Option<String> {
    if solidcraft_ui_text::language() != "zh" {
        return None;
    }
    match axis {
        "load_factor" => with_unit(raw, "Load factor", "载荷因子"),
        "time" => with_unit(raw, "Time", "时间"),
        "mode" => with_unit(raw, "Mode", "振型阶次"),
        "displacement" => {
            let (en, zh) = quantity(match x["component"].as_str().unwrap_or("magnitude") {
                "x" => "displacement_x",
                "y" => "displacement_y",
                "z" => "displacement_z",
                "magnitude" => "displacement",
                _ => return None,
            })?;
            if let Some(n) = x["displacement_node"].as_u64() {
                Some(format!("节点 {n}：{}", with_unit(raw.strip_suffix(&format!(" at node {n}"))?, en, zh)?))
            } else {
                let set = x["displacement"].as_str()?;
                Some(format!("{set}：{}（平均值）", with_unit(raw.strip_suffix(&format!(" of {set} (mean)"))?, en, zh)?))
            }
        }
        _ => None,
    }
}

pub fn caption(y: &Value, raw: &str) -> Option<String> {
    if solidcraft_ui_text::language() != "zh" {
        return None;
    }
    if let Some(set) = y["reaction"].as_str() {
        let (en, zh) = match y["component"].as_str().unwrap_or("magnitude") {
            "x" => ("Reaction X", "X 向反力"),
            "y" => ("Reaction Y", "Y 向反力"),
            "z" => ("Reaction Z", "Z 向反力"),
            "magnitude" => ("Reaction magnitude", "反力幅值"),
            _ => return None,
        };
        return Some(format!("{set}：{}", with_unit(raw.strip_suffix(&format!(" on {set}"))?, en, zh)?));
    }
    if let Some(n) = y["history_node"].as_u64() {
        let (en, zh) = quantity(y["quantity"].as_str()?)?;
        return Some(format!("节点 {n} 处的{}", with_unit(raw.strip_suffix(&format!(" at node {n}"))?, en, zh)?));
    }
    let (en, zh) = quantity(y.as_str().or_else(|| y["energy"].as_str()).or_else(|| y["history"].as_str())?)?;
    with_unit(raw, en, zh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn localized_display_axis_captions_preserve_names_and_units() {
        crate::theme::install_look();
        let x = json!({"displacement":"Time","component":"x"});
        let raw = "Displacement X [in] of Time (mean)";
        {
            let _lang = solidcraft_ui_text::scope("zh");
            assert_eq!(axis_caption("displacement", &x, raw).as_deref(), Some("Time：X 向位移 [in]（平均值）"));
            assert_eq!(axis_caption("time", &Value::Null, "Time [s]").as_deref(), Some("时间 [s]"));
            assert!(axis_caption("custom", &Value::Null, "Time").is_none());
        }
        let _lang = solidcraft_ui_text::scope("en");
        assert!(axis_caption("displacement", &x, raw).is_none());
    }
    #[test]
    fn series_labels_preserve_units_and_set_names() {
        crate::theme::install_look();
        let _lang = solidcraft_ui_text::scope("zh");
        assert_eq!(caption(&json!({"reaction":"Faces","component":"x"}), "Reaction X [N] on Faces").as_deref(), Some("Faces：X 向反力 [N]"));
        assert_eq!(caption(&json!({"energy":"kinetic"}), "Kinetic energy [J]").as_deref(), Some("动能 [J]"));
        assert_eq!(
            caption(&json!({"history_node":4,"quantity":"velocity_z"}), "Velocity Z [m/s] at node 4").as_deref(),
            Some("节点 4 处的Z 向速度 [m/s]")
        );
    }
}
