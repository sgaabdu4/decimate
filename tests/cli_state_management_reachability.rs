use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn bloc_cubit_and_provider_references_keep_state_owners_live()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_state_management_fixture(&fixture)?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--include-entry-exports",
    ])?;

    assert_eq!(code, 1);
    for symbol in [
        "CounterEvent",
        "CounterStarted",
        "CounterState",
        "CounterBloc",
        "CounterCubit",
        "CartModel",
    ] {
        assert_unused_export_absent(&json, symbol);
    }
    assert_unused_export_present(&json, "UnusedStateOwner");

    Ok(())
}

#[test]
fn long_generated_riverpod_annotations_keep_source_owner_live()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "\
name: app
dependencies:
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

@Riverpod(
  keepAlive: true,
  dependencies: [
    dep1,
    dep2,
    dep3,
    dep4,
    dep5,
    dep6,
    dep7,
    dep8,
    dep9,
    dep10,
  ],
)
Future<int> fetchProducts(Ref ref) async => 1;

class Ref {}
const dep1 = Object();
const dep2 = Object();
const dep3 = Object();
const dep4 = Object();
const dep5 = Object();
const dep6 = Object();
const dep7 = Object();
const dep8 = Object();
const dep9 = Object();
const dep10 = Object();
class UnusedService {}
",
    )?;
    write(
        &fixture,
        "lib/providers.g.dart",
        "\
part of 'providers.dart';
final fetchProductsProvider = Object();
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--include-entry-exports",
    ])?;

    assert_eq!(code, 1);
    assert_unused_export_absent(&json, "fetchProducts");
    assert_unused_export_present(&json, "UnusedService");

    Ok(())
}

fn assert_unused_export_absent(json: &Value, name: &str) {
    assert!(
        !unused_exports(json).any(|finding| finding_targets_symbol(finding, name)),
        "{name} should be counted as used by state-management graph references"
    );
}

fn assert_unused_export_present(json: &Value, name: &str) {
    assert!(
        unused_exports(json).any(|finding| finding_targets_symbol(finding, name)),
        "{name} should still be reported when it has no graph reference"
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

fn write_state_management_fixture(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", STATE_MANAGEMENT_PUBSPEC)?;
    write(fixture, "lib/main.dart", STATE_MANAGEMENT_MAIN)?;
    write(fixture, "lib/state.dart", STATE_MANAGEMENT_STATE)
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

const STATE_MANAGEMENT_PUBSPEC: &str = "\
name: app
dependencies:
  bloc: any
  flutter:
    sdk: flutter
  flutter_bloc: any
  provider: any
";

const STATE_MANAGEMENT_MAIN: &str = "\
import 'state.dart';
void main() {
  BlocBuilder<CounterBloc, CounterState>(builder: (_, state) => const SizedBox());
  BlocConsumer<CounterBloc, CounterState>(
    listener: (_, state) {},
    builder: (_, state) => const SizedBox(),
  );
  BlocListener<CounterBloc, CounterState>(listener: (_, state) {});
  BlocSelector<CounterBloc, CounterState, int>(
    selector: (state) => 0,
    builder: (_, count) => const SizedBox(),
  );
  BlocProvider(create: (_) => CounterCubit());
  BlocBuilder<CounterCubit, int>(builder: (_, state) => const SizedBox());
  MultiBlocProvider(providers: [BlocProvider(create: (_) => CounterBloc())], child: const SizedBox());
  MultiBlocListener(
    listeners: [BlocListener<CounterBloc, CounterState>(listener: (_, state) {})],
    child: const SizedBox(),
  );
  context.read<CounterBloc>().add(CounterStarted());
  context.read<CounterCubit>().increment();
  context.watch<CartModel>();
  context.select((CartModel model) => model.count);
  Provider.of<CartModel>(context);
  ChangeNotifierProvider(create: (_) => CartModel(), child: const SizedBox());
  Selector<CartModel, int>(
    selector: (_, model) => model.count,
    builder: (_, count, child) => child!,
  );
  Consumer<CartModel>(builder: (_, model, child) => child!);
  Consumer2<CartModel, CounterCubit>(builder: (_, cart, cubit, child) => child!);
}
final context = Context();
class Context {
  T read<T>() => throw UnimplementedError();
  T watch<T>() => throw UnimplementedError();
  R select<T, R>(R Function(T) selector) => throw UnimplementedError();
}
class BlocBuilder<B, S> {
  const BlocBuilder({required Object Function(Object, S) builder});
}
class BlocConsumer<B, S> {
  const BlocConsumer({
    required void Function(Object, S) listener,
    required Object Function(Object, S) builder,
  });
}
class BlocListener<B, S> {
  const BlocListener({required void Function(Object, S) listener});
}
class BlocSelector<B, S, T> {
  const BlocSelector({required T Function(S) selector, required Object Function(Object, T) builder});
}
class BlocProvider {
  const BlocProvider({required Object Function(Object) create});
}
class MultiBlocProvider {
  const MultiBlocProvider({required List<Object> providers, required Object child});
}
class MultiBlocListener {
  const MultiBlocListener({required List<Object> listeners, required Object child});
}
class Provider {
  static T of<T>(Object context) => throw UnimplementedError();
}
class ChangeNotifierProvider {
  const ChangeNotifierProvider({required Object Function(Object) create, required Object child});
}
class Selector<T, S> {
  const Selector({required S Function(Object, T) selector, required Object? Function(Object, S, Object?) builder});
}
class Consumer<T> {
  const Consumer({required Object? Function(Object, T, Object?) builder});
}
class Consumer2<A, B> {
  const Consumer2({required Object? Function(Object, A, B, Object?) builder});
}
class SizedBox {
  const SizedBox();
}
";

const STATE_MANAGEMENT_STATE: &str = "\
class Bloc<E, S> {}
class Cubit<S> {}
class ChangeNotifier {}

class CounterEvent {}
class CounterStarted extends CounterEvent {}
class CounterState {}

class CounterBloc extends Bloc<CounterEvent, CounterState> {
  void add(CounterEvent event) {}
}

class CounterCubit extends Cubit<int> {
  void increment() {}
}

class CartModel extends ChangeNotifier {
  int get count => 0;
}

class UnusedStateOwner {}
";
