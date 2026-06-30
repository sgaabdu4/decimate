use super::*;

#[test]
fn decodes_non_raw_dart_string_escapes_in_directive_uris() -> Result<(), ExtractError> {
    let source = "\
import 'src/\\u0061pi.dart';
export \"src/\\u{66}eature.dart\";
part 'src/model\\u{20}part.dart';
";

    let extracted = extract_dart_source("lib/main.dart", source)?;

    assert_eq!(extracted.imports[0].uri, "src/api.dart");
    assert_eq!(extracted.exports[0].uri, "src/feature.dart");
    assert_eq!(extracted.parts[0].uri, "src/model part.dart");

    Ok(())
}
