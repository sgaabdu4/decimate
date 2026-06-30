use std::path::Path;

use super::*;

#[test]
fn flags_unused_stateless_widget_field_formal() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required this.title, required this.subtitle});
  final String title;
  final String subtitle;
  Widget build(BuildContext context) => Text(title);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "UserCard");
    assert_eq!(unused[0].param_name, "subtitle");
    assert_eq!(unused[0].location.line, 3);
    Ok(())
}

#[test]
fn flags_unused_explicit_widget_constructor_params() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String unused, required String used})
      : unused = unused,
        used = used;
  final String unused;
  final String used;
  Widget build(BuildContext context) => Text(used);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "UserCard");
    assert_eq!(unused[0].param_name, "unused");
    assert_eq!(unused[0].location.line, 3);
    Ok(())
}

#[test]
fn respects_explicit_params_used_through_backing_fields_and_state()
-> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String label}) : _label = label;
  final String _label;
  Widget build(BuildContext context) => Text(_label);
}
class CounterCard extends StatefulWidget {
  const CounterCard({super.key, required int count}) : _count = count;
  final int _count;
  State<CounterCard> createState() => _CounterCardState();
}
class _CounterCardState extends State<CounterCard> {
  Widget build(BuildContext context) => Text('${widget._count}');
}
";
    let unused = parse_findings(source)?.unused_params;

    assert!(unused.is_empty(), "{unused:?}");
    Ok(())
}

#[test]
fn flags_explicit_params_when_backing_field_is_unused() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required String subtitle}) : _subtitle = subtitle;
  final String _subtitle;
  Widget build(BuildContext context) => const SizedBox();
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].param_name, "subtitle");
    Ok(())
}

#[test]
fn respects_widget_and_state_usages() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class UsedInBuild extends StatelessWidget {
  const UsedInBuild({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text('$title');
}
class UsedViaState extends StatefulWidget {
  const UsedViaState({super.key, required this.count});
  final int count;
  State<UsedViaState> createState() => _UsedViaStateState();
}
class _UsedViaStateState extends State<UsedViaState> {
  Widget build(BuildContext context) => Text('${widget.count}');
}
";
    let unused = parse_findings(source)?.unused_params;

    assert!(unused.is_empty(), "{unused:?}");
    Ok(())
}

#[test]
fn recognizes_consumer_and_hook_widget_bases() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class A extends ConsumerWidget {
  const A({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}
class B extends HookConsumerWidget {
  const B({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => Text(value);
}
class C extends ConsumerStatefulWidget {
  const C({super.key, required this.value});
  final String value;
  ConsumerState<C> createState() => _CState();
}
class _CState extends ConsumerState<C> {
  Widget build(BuildContext context) => Text(oldWidget.value);
}
";
    let unused = parse_findings(source)?.unused_params;

    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0].widget_class, "A");
    assert_eq!(unused[0].widget_kind, WidgetClassKind::ConsumerWidget);
    Ok(())
}

#[test]
fn flags_private_widget_classes_but_allows_private_states() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class PublicCard extends StatefulWidget {
  State<PublicCard> createState() => _PublicCardState();
}
class _PublicCardState extends State<PublicCard> {}
class PublicConsumer extends ConsumerStatefulWidget {
  ConsumerState<PublicConsumer> createState() => _PublicConsumerState();
}
class _PublicConsumerState extends ConsumerState<PublicConsumer> {}
class _PrivateCard extends StatelessWidget {}
class _PrivateShell extends ConsumerWidget {}
class _PrivateHook extends HookConsumerWidget {}
";
    let private_widgets = parse_findings(source)?.private_widget_classes;

    let classes = private_widgets
        .iter()
        .map(|widget| widget.widget_class.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        classes,
        vec!["_PrivateCard", "_PrivateShell", "_PrivateHook"]
    );
    Ok(())
}

#[test]
fn reports_all_private_widget_base_kinds() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class _A extends StatelessWidget {}
class _B extends StatefulWidget {}
class _C extends ConsumerWidget {}
class _D extends ConsumerStatefulWidget {}
class _E extends HookWidget {}
class _F extends HookConsumerWidget {}
";
    let kinds = parse_findings(source)?
        .private_widget_classes
        .into_iter()
        .map(|widget| widget.widget_kind)
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            WidgetClassKind::StatelessWidget,
            WidgetClassKind::StatefulWidget,
            WidgetClassKind::ConsumerWidget,
            WidgetClassKind::ConsumerStatefulWidget,
            WidgetClassKind::HookWidget,
            WidgetClassKind::HookConsumerWidget,
        ]
    );
    Ok(())
}

#[test]
fn ignores_public_widgets_and_non_widget_private_classes() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class PublicCard extends StatelessWidget {}
class _Formatter {}
";
    let private_widgets = parse_findings(source)?.private_widget_classes;

    assert!(private_widgets.is_empty(), "{private_widgets:?}");
    Ok(())
}

#[test]
fn flags_top_level_widget_helper_functions_in_widget_files()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "\
class App extends StatelessWidget {}
Widget _buildHeader(BuildContext context) => const SizedBox();
List<Widget> buildItems() => const [];
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert_eq!(helpers.len(), 2);
    assert_eq!(helpers[0].function_name, "_buildHeader");
    assert_eq!(helpers[0].return_type.as_deref(), Some("Widget"));
    assert_eq!(helpers[0].location.line, 2);
    assert_eq!(helpers[1].function_name, "buildItems");
    Ok(())
}

#[test]
fn flags_top_level_helpers_in_screen_files() -> Result<(), Box<dyn std::error::Error>> {
    let source = "Widget header(BuildContext context) => const SizedBox();\n";
    let helpers = parse_findings_at("lib/screens/home_screen.dart", source)?.top_level_functions;

    assert_eq!(helpers.len(), 1);
    assert_eq!(helpers[0].function_name, "header");
    Ok(())
}

#[test]
fn does_not_flag_methods_local_functions_or_namespaces() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class App extends StatefulWidget {}
class _AppState extends State<App> {
  Widget _buildHeader(BuildContext context) => const SizedBox();
}
abstract final class AppParts {
  static Widget header(BuildContext context) => const SizedBox();
}
void container() {
  Widget _buildLocal(BuildContext context) => const SizedBox();
}
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert!(helpers.is_empty(), "{helpers:?}");
    Ok(())
}

#[test]
fn does_not_flag_main_providers_or_widget_named_configs() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class App extends StatelessWidget {}
void main() {}
int count(Ref ref) => 1;
String title() => 'ok';
MyWidgetConfig _buildConfig() => MyWidgetConfig();
WidgetBuilder makeBuilder() => (context) => const SizedBox();
";
    let helpers = parse_findings(source)?.top_level_functions;

    assert!(helpers.is_empty(), "{helpers:?}");
    Ok(())
}

#[test]
fn flags_manual_riverpod_provider_declarations() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:flutter_riverpod/flutter_riverpod.dart';

final counterProvider = StateProvider<int>((ref) => 0);
final userProvider = FutureProvider.autoDispose.family<String, int>((ref, id) async => '$id');
";
    let providers = parse_findings(source)?.manual_riverpod_providers;

    assert_eq!(providers.len(), 2);
    assert_eq!(providers[0].provider_name, "counterProvider");
    assert_eq!(providers[0].provider_type, "StateProvider");
    assert_eq!(providers[0].location.line, 4);
    assert_eq!(providers[1].provider_name, "userProvider");
    assert_eq!(providers[1].provider_type, "FutureProvider");
    Ok(())
}

#[test]
fn flags_prefixed_manual_riverpod_providers() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
import 'package:riverpod/riverpod.dart' as riverpod;

final titleProvider = riverpod.Provider<String>((ref) => 'title');
";
    let providers = parse_findings(source)?.manual_riverpod_providers;

    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].provider_name, "titleProvider");
    assert_eq!(providers[0].provider_type, "Provider");
    Ok(())
}

#[test]
fn ignores_manual_provider_shapes_without_riverpod_import_or_top_level_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let no_import = r"
class Provider<T> {}
final localProvider = Provider<int>();
";
    assert!(
        parse_findings(no_import)?
            .manual_riverpod_providers
            .is_empty()
    );

    let local_scope = r"
import 'package:flutter_riverpod/flutter_riverpod.dart';

void main() {
  final localProvider = Provider<int>((ref) => 1);
  localProvider;
}
";
    assert!(
        parse_findings(local_scope)?
            .manual_riverpod_providers
            .is_empty()
    );
    Ok(())
}

#[test]
fn flags_widget_awaits_without_context_mounted_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatefulWidget {
  State<SaveButton> createState() => _SaveButtonState();
}

class _SaveButtonState extends State<SaveButton> {
  Future<void> save() async {
    await doWork();
    Navigator.of(context).pop();
  }

  Future<void> guarded() async {
    await doWork();
    if (!context.mounted) return;
    Navigator.of(context).pop();
  }

  Future<void> bareMountedIsNotEnough() async {
    await doWork();
    if (!mounted) return;
    Navigator.of(context).pop();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].owner, "_SaveButtonState.save");
    assert_eq!(findings[0].location.line, 8);
    assert_eq!(findings[1].owner, "_SaveButtonState.bareMountedIsNotEnough");
    Ok(())
}

#[test]
fn flags_nested_widget_awaits_per_block() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class SaveButton extends StatelessWidget {
  Future<void> save(BuildContext context, bool active) async {
    if (active) {
      await doWork();
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
    await doWork();
  }
}
";
    let findings = parse_findings(source)?.missing_context_mounted_after_await;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].owner, "SaveButton.save");
    assert_eq!(findings[0].location.line, 9);
    Ok(())
}

#[test]
fn flags_notifier_awaits_without_ref_mounted_guard() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class CounterNotifier extends _$CounterNotifier {
  int build() => 0;

  Future<void> save() async {
    await repo.save();
    state++;
  }

  Future<void> guarded() async {
    await repo.save();
    if (!ref.mounted) return;
    state++;
  }

  Future<int> terminal() async {
    return await repo.count();
  }
}
";
    let findings = parse_findings(source)?.missing_ref_mounted_after_await;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].owner, "CounterNotifier.save");
    assert_eq!(findings[0].location.line, 6);
    Ok(())
}

#[test]
fn recognizes_generated_riverpod_notifier_superclasses() -> Result<(), Box<dyn std::error::Error>> {
    let source = r"
class Counter extends _$Counter {
  int build() => 0;

  Future<void> save() async {
    await repo.save();
    state++;
  }
}
";
    let findings = parse_findings(source)?.missing_ref_mounted_after_await;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].owner, "Counter.save");
    Ok(())
}

#[test]
fn flags_ref_watch_inside_notifier_methods_except_build() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r"
class CounterNotifier extends _$CounterNotifier {
  int build() {
    return ref.watch(counterProvider);
  }

  void save() {
    final value = ref.watch(counterProvider);
    state = value;
  }
}

class CounterWidget extends ConsumerWidget {
  Widget build(BuildContext context, WidgetRef ref) {
    final value = ref.watch(counterProvider);
    return Text('$value');
  }
}
";
    let findings = parse_findings(source)?.riverpod_watch_in_notifier_methods;

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].notifier_class, "CounterNotifier");
    assert_eq!(findings[0].method_name, "save");
    assert_eq!(findings[0].location.line, 8);
    Ok(())
}

fn parse_findings(source: &str) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    parse_findings_at("lib/widgets.dart", source)
}

fn parse_findings_at(path: &str, source: &str) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    let path = Path::new(path);
    let parsed = parse_tree(path, source)?;
    Ok(findings_in_source(
        path,
        parsed.tree().root_node(),
        parsed.source(),
    ))
}
