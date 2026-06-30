use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn generated_riverpod_provider_references_keep_source_owners_live()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "\
name: app
dependencies:
  flutter_riverpod: any
  riverpod_annotation: any
dev_dependencies:
  riverpod_generator: any
",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "\
import 'providers.dart';
void main() {
  ref.watch(fetchProductsProvider);
  ref.watch(counterProvider);
}
final ref = Ref();
class Ref {
  void watch(Object provider) {}
}
",
    )?;
    write(
        &fixture,
        "lib/providers.dart",
        "\
import 'package:riverpod_annotation/riverpod_annotation.dart';
part 'providers.g.dart';

@riverpod
Future<int> fetchProducts(Ref ref) async => 1;

@Riverpod(keepAlive: true)
class Counter extends _$Counter {
  int build() => 0;
}

class Ref {}
class _$Counter {}
class UnusedService {}
",
    )?;
    write(
        &fixture,
        "lib/providers.g.dart",
        "\
part of 'providers.dart';
final fetchProductsProvider = Object();
final counterProvider = Object();
",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--include-entry-exports",
    ])?;

    assert_eq!(code, 1);
    assert_unused_export_absent(&json, "fetchProducts");
    assert_unused_export_absent(&json, "Counter");
    assert_unused_export_present(&json, "UnusedService");

    Ok(())
}

fn assert_unused_export_absent(json: &Value, name: &str) {
    assert!(
        !unused_exports(json).any(|finding| finding_targets_symbol(finding, name)),
        "{name} should be counted as used by its generated provider"
    );
}

fn assert_unused_export_present(json: &Value, name: &str) {
    assert!(
        unused_exports(json).any(|finding| finding_targets_symbol(finding, name)),
        "{name} should still be reported when it has no generated provider reference"
    );
}

fn unused_exports(json: &Value) -> impl Iterator<Item = &Value> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["kind"] == "unused-export")
}

fn finding_targets_symbol(finding: &Value, name: &str) -> bool {
    finding["actions"]
        .as_array()
        .is_some_and(|actions| actions.iter().any(|action| action["target_symbol"] == name))
}

fn run_json<I, S>(args: I) -> Result<(i32, Value), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
