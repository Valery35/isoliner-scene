// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tool_spec::{Lang, ParamKind, Value, parse_catalog};

fn catalog() -> tool_spec::Catalog {
    let path = format!("{}/../../fixtures/tools.json", env!("CARGO_MANIFEST_DIR"));
    parse_catalog(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn reads_the_module_catalog() {
    let cat = catalog();
    assert_eq!(cat.module, "isoliner3d");
    let fill = cat.tool("fill_to_volume").unwrap();
    assert_eq!(
        fill.display_name(Lang::Ru),
        "2.23 Заполнение тела до объёма"
    );
    assert_eq!(
        fill.display_name(Lang::En),
        "2.23 Fill a body up to a volume"
    );
    assert_eq!(fill.param("SHELL").unwrap().kind, ParamKind::Mesh);
    let sweep = cat.tool("sweep_body").unwrap();
    assert_eq!(
        sweep.param("WIDTH_FIELD").unwrap().parent.as_deref(),
        Some("INPUT")
    );
    assert_eq!(
        sweep.translate("Незамкнутых тел: %d.", Lang::En),
        "Open bodies: %d."
    );
    assert_eq!(sweep.translate("Круг", Lang::En), "Circle");
    for t in &cat.tools {
        for p in &t.params {
            assert!(!p.hint.ru.is_empty() && !p.hint.en.is_empty());
        }
    }
}

#[test]
fn defaults_and_checks() {
    let cat = catalog();
    let t = cat.tool("sweep_body").unwrap();
    let mut v = t.defaults();
    assert_eq!(v.get("SEGMENTS"), Some(&Value::Integer(16)));
    assert_eq!(v.get("PROFILE"), Some(&Value::Enum(0)));
    let bad = t.check(&v, Lang::Ru);
    assert_eq!(bad.len(), 1, "{bad:?}"); // нет линий
    v.insert("INPUT".into(), Value::Lines(vec![]));
    assert!(t.check(&v, Lang::Ru).is_empty());
    v.insert("SEGMENTS".into(), Value::Integer(500));
    v.insert("PROFILE".into(), Value::Enum(7));
    assert_eq!(t.check(&v, Lang::Ru).len(), 2);
}
