pub(super) fn strip_json_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                output.push(ch);
            }
            '/' if chars.peek() == Some(&'/') => {
                let _ = chars.next();
                skip_line_comment(&mut chars, &mut output);
            }
            '/' if chars.peek() == Some(&'*') => {
                let _ = chars.next();
                skip_block_comment(&mut chars, &mut output);
            }
            _ => output.push(ch),
        }
    }

    output
}

fn skip_line_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    for ch in chars.by_ref() {
        if ch == '\n' {
            output.push('\n');
            break;
        }
    }
}

fn skip_block_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    let mut previous = '\0';
    for ch in chars.by_ref() {
        if ch == '\n' {
            output.push('\n');
        }
        if previous == '*' && ch == '/' {
            break;
        }
        previous = ch;
    }
}
