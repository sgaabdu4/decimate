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

fn parse_findings(source: &str) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    let tree = parse_tree(Path::new("lib/widgets.dart"), source)?;
    Ok(findings_in_source(
        Path::new("lib/widgets.dart"),
        tree.root_node(),
        source,
    ))
}
