use std::collections::BTreeMap;

use crate::{DartExport, DartImport, DartUriCondition, Location};

pub(super) fn selected_imports<'a>(
    imports: &'a [DartImport],
    environment: &BTreeMap<String, String>,
) -> Vec<&'a DartImport> {
    if environment.is_empty() {
        return imports.iter().collect();
    }
    select_configurable_items(imports, environment, |import| {
        (import.location, import.condition.as_ref())
    })
}

pub(super) fn selected_exports<'a>(
    exports: &'a [DartExport],
    environment: &BTreeMap<String, String>,
) -> Vec<&'a DartExport> {
    if environment.is_empty() {
        return exports.iter().collect();
    }
    select_configurable_items(exports, environment, |export| {
        (export.location, export.condition.as_ref())
    })
}

fn select_configurable_items<'a, T>(
    items: &'a [T],
    environment: &BTreeMap<String, String>,
    metadata: impl Fn(&'a T) -> (Location, Option<&'a DartUriCondition>),
) -> Vec<&'a T> {
    let mut selected = Vec::new();
    let mut index = 0;

    while index < items.len() {
        let (location, first_condition) = metadata(&items[index]);
        let mut end = index + 1;
        let mut has_condition = first_condition.is_some();
        while end < items.len() {
            let (next_location, next_condition) = metadata(&items[end]);
            if next_location != location {
                break;
            }
            has_condition |= next_condition.is_some();
            end += 1;
        }

        let group = &items[index..end];
        if !has_condition {
            selected.extend(group.iter());
        } else if let Some(item) = group.iter().find(|item| {
            metadata(item)
                .1
                .is_some_and(|condition| condition_matches(condition, environment))
        }) {
            selected.push(item);
        } else if let Some(default_item) = group.iter().find(|item| metadata(item).1.is_none()) {
            selected.push(default_item);
        }

        index = end;
    }

    selected
}

fn condition_matches(condition: &DartUriCondition, environment: &BTreeMap<String, String>) -> bool {
    environment
        .get(&condition.variable)
        .map_or("false", String::as_str)
        == condition.expected_value
}
