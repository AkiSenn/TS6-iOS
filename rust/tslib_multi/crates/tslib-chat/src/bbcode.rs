//! BBCode parsing and rendering

use regex::Regex;

/// BBCode parser
pub struct BBCodeParser;

/// BBCode renderer for different output formats
pub trait BBCodeRenderer {
    /// Render bold text
    fn bold(&self, content: &str) -> String;
    /// Render italic text
    fn italic(&self, content: &str) -> String;
    /// Render underlined text
    fn underline(&self, content: &str) -> String;
    /// Render strikethrough text
    fn strike(&self, content: &str) -> String;
    /// Render colored text
    fn color(&self, color: &str, content: &str) -> String;
    /// Render sized text
    fn size(&self, size: &str, content: &str) -> String;
    /// Render a URL
    fn url(&self, url: &str, text: &str) -> String;
    /// Render an image
    fn image(&self, url: &str) -> String;
    /// Render inline code
    fn code(&self, content: &str) -> String;
    /// Render a code block
    fn code_block(&self, content: &str) -> String;
}

/// HTML renderer
pub struct HtmlRenderer;

impl BBCodeRenderer for HtmlRenderer {
    fn bold(&self, content: &str) -> String {
        format!("<strong>{}</strong>", content)
    }

    fn italic(&self, content: &str) -> String {
        format!("<em>{}</em>", content)
    }

    fn underline(&self, content: &str) -> String {
        format!("<u>{}</u>", content)
    }

    fn strike(&self, content: &str) -> String {
        format!("<s>{}</s>", content)
    }

    fn color(&self, color: &str, content: &str) -> String {
        format!("<span style=\"color: {}\">{}</span>", color, content)
    }

    fn size(&self, size: &str, content: &str) -> String {
        format!("<span style=\"font-size: {}px\">{}</span>", size, content)
    }

    fn url(&self, url: &str, text: &str) -> String {
        format!("<a href=\"{}\">{}</a>", url, text)
    }

    fn image(&self, url: &str) -> String {
        format!("<img src=\"{}\" />", url)
    }

    fn code(&self, content: &str) -> String {
        format!("<code>{}</code>", content)
    }

    fn code_block(&self, content: &str) -> String {
        format!("<pre><code>{}</code></pre>", content)
    }
}

/// Plain text renderer (strips BBCode)
pub struct PlainTextRenderer;

impl BBCodeRenderer for PlainTextRenderer {
    fn bold(&self, content: &str) -> String {
        content.to_string()
    }

    fn italic(&self, content: &str) -> String {
        content.to_string()
    }

    fn underline(&self, content: &str) -> String {
        content.to_string()
    }

    fn strike(&self, content: &str) -> String {
        content.to_string()
    }

    fn color(&self, _color: &str, content: &str) -> String {
        content.to_string()
    }

    fn size(&self, _size: &str, content: &str) -> String {
        content.to_string()
    }

    fn url(&self, url: &str, text: &str) -> String {
        if text == url {
            url.to_string()
        } else {
            format!("{} ({})", text, url)
        }
    }

    fn image(&self, url: &str) -> String {
        format!("[Image: {}]", url)
    }

    fn code(&self, content: &str) -> String {
        content.to_string()
    }

    fn code_block(&self, content: &str) -> String {
        content.to_string()
    }
}

/// ANSI terminal renderer
pub struct AnsiRenderer;

impl BBCodeRenderer for AnsiRenderer {
    fn bold(&self, content: &str) -> String {
        format!("\x1b[1m{}\x1b[0m", content)
    }

    fn italic(&self, content: &str) -> String {
        format!("\x1b[3m{}\x1b[0m", content)
    }

    fn underline(&self, content: &str) -> String {
        format!("\x1b[4m{}\x1b[0m", content)
    }

    fn strike(&self, content: &str) -> String {
        format!("\x1b[9m{}\x1b[0m", content)
    }

    fn color(&self, color: &str, content: &str) -> String {
        // Simple color mapping
        let ansi_code = match color.to_lowercase().as_str() {
            "red" => "31",
            "green" => "32",
            "yellow" => "33",
            "blue" => "34",
            "magenta" => "35",
            "cyan" => "36",
            "white" => "37",
            _ => "39", // default
        };
        format!("\x1b[{}m{}\x1b[0m", ansi_code, content)
    }

    fn size(&self, _size: &str, content: &str) -> String {
        content.to_string()
    }

    fn url(&self, url: &str, text: &str) -> String {
        // OSC 8 hyperlink if supported
        format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
    }

    fn image(&self, url: &str) -> String {
        format!("[Image: {}]", url)
    }

    fn code(&self, content: &str) -> String {
        format!("\x1b[7m{}\x1b[0m", content)
    }

    fn code_block(&self, content: &str) -> String {
        format!("\x1b[2m{}\x1b[0m", content)
    }
}

impl BBCodeParser {
    /// Parse and render BBCode using the given renderer.
    /// Preprocesses Markdown syntax (code, bold, italic, strikethrough)
    /// into BBCode equivalents before applying the BBCode pipeline.
    pub fn render<R: BBCodeRenderer>(input: &str, renderer: &R) -> String {
        let (mut output, protected) = Self::preprocess_markdown(input, renderer);

        // Process tags from innermost to outermost
        output = Self::process_simple_tag(&output, "b", |c| renderer.bold(c));
        output = Self::process_simple_tag(&output, "i", |c| renderer.italic(c));
        output = Self::process_simple_tag(&output, "u", |c| renderer.underline(c));
        output = Self::process_simple_tag(&output, "s", |c| renderer.strike(c));

        output = Self::process_param_tag(&output, "color", |p, c| renderer.color(p, c));
        output = Self::process_param_tag(&output, "size", |p, c| renderer.size(p, c));
        output = Self::process_param_tag(&output, "url", |p, c| renderer.url(p, c));

        // Simple URL tags
        output = Self::process_simple_tag(&output, "url", |c| renderer.url(c, c));

        // Images
        output = Self::process_simple_tag(&output, "img", |c| renderer.image(c));

        // Restore protected code regions
        for (placeholder, rendered) in &protected {
            output = output.replace(placeholder.as_str(), rendered.as_str());
        }

        output
    }

    /// Preprocess Markdown syntax into BBCode equivalents.
    /// Code regions are protected via NUL-byte placeholders to prevent
    /// further processing by the BBCode pipeline.
    fn preprocess_markdown<R: BBCodeRenderer>(
        input: &str,
        renderer: &R,
    ) -> (String, Vec<(String, String)>) {
        let mut output = input.to_string();
        let mut protected: Vec<(String, String)> = Vec::new();

        // 1. Fenced code blocks: ```lang\n...\n```
        let re_code_block = Regex::new(r"(?s)```(?:\w*)\n?(.*?)```").unwrap();
        output = re_code_block
            .replace_all(&output, |caps: &regex::Captures| {
                let idx = protected.len();
                let placeholder = format!("\x00CODE_BLOCK_{}\x00", idx);
                let rendered = renderer.code_block(&caps[1]);
                protected.push((placeholder.clone(), rendered));
                placeholder
            })
            .to_string();

        // 2. Inline code: `...`
        let re_inline_code = Regex::new(r"`([^`\n]+)`").unwrap();
        output = re_inline_code
            .replace_all(&output, |caps: &regex::Captures| {
                let idx = protected.len();
                let placeholder = format!("\x00INLINE_CODE_{}\x00", idx);
                let rendered = renderer.code(&caps[1]);
                protected.push((placeholder.clone(), rendered));
                placeholder
            })
            .to_string();

        // 3. Markdown tables → code block (protected)
        {
            let lines: Vec<&str> = output.lines().collect();
            let mut result_lines: Vec<String> = Vec::new();
            let re_table_sep = Regex::new(r"^\|(\s*:?-+:?\s*\|)+\s*$").unwrap();
            let mut i = 0;
            while i < lines.len() {
                if lines[i].starts_with('|') {
                    let start = i;
                    while i < lines.len() && lines[i].starts_with('|') {
                        i += 1;
                    }
                    if i - start >= 2 {
                        let table_lines: Vec<&str> = lines[start..i]
                            .iter()
                            .filter(|l| !re_table_sep.is_match(l))
                            .copied()
                            .collect();
                        let table_content = table_lines.join("\n");
                        let idx = protected.len();
                        let placeholder = format!("\x00TABLE_{}\x00", idx);
                        let rendered = renderer.code_block(&table_content);
                        protected.push((placeholder.clone(), rendered));
                        result_lines.push(placeholder);
                    } else {
                        for line in &lines[start..i] {
                            result_lines.push(line.to_string());
                        }
                    }
                } else {
                    result_lines.push(lines[i].to_string());
                    i += 1;
                }
            }
            output = result_lines.join("\n");
        }

        // 4. Markdown images: ![alt](url) or ![alt](url "title") → [img]url[/img]
        let re_md_image = Regex::new(r#"!\[([^\]]*)\]\((\S+?)(?:\s+"[^"]*")?\)"#).unwrap();
        output = re_md_image.replace_all(&output, "[img]$2[/img]").to_string();

        // 5. Markdown links: [text](url) → [url=url]text[/url]
        let re_md_link = Regex::new(r#"\[([^\]]+)\]\((https?://[^\)]+)\)"#).unwrap();
        output = re_md_link.replace_all(&output, "[url=$2]$1[/url]").to_string();

        // 6. Checkboxes (before list items)
        let re_cb_checked = Regex::new(r"(?m)^(\s*)[-*] \[x\] (.+)").unwrap();
        output = re_cb_checked.replace_all(&output, "$1☑ $2").to_string();
        let re_cb_unchecked = Regex::new(r"(?m)^(\s*)[-*] \[ ?\] (.+)").unwrap();
        output = re_cb_unchecked.replace_all(&output, "$1☐ $2").to_string();

        // 7. Unordered list items: "- text" or "* text" at start of line → bullet
        let re_list = Regex::new(r"(?m)^(\s*)[-*] (.+)").unwrap();
        output = re_list.replace_all(&output, "$1• $2").to_string();

        // 8. Bold+Italic: ***text*** and ___text___
        let re_bold_italic_star = Regex::new(r"\*\*\*(.+?)\*\*\*").unwrap();
        output = re_bold_italic_star.replace_all(&output, "[b][i]$1[/i][/b]").to_string();
        let re_bold_italic_under = Regex::new(r"\b___(.+?)___\b").unwrap();
        output = re_bold_italic_under.replace_all(&output, "[b][i]$1[/i][/b]").to_string();

        // 9. Bold: **text** and __text__
        let re_bold_star = Regex::new(r"\*\*(.+?)\*\*").unwrap();
        output = re_bold_star.replace_all(&output, "[b]$1[/b]").to_string();
        let re_bold_under = Regex::new(r"\b__(.+?)__\b").unwrap();
        output = re_bold_under.replace_all(&output, "[b]$1[/b]").to_string();

        // 10. Italic: *text* and _text_ (\b prevents matching inside identifiers)
        let re_italic_star = Regex::new(r"\*(\S(?:.*?\S)?)\*").unwrap();
        output = re_italic_star.replace_all(&output, "[i]$1[/i]").to_string();
        let re_italic_under = Regex::new(r"\b_(\S(?:.*?\S)?)_\b").unwrap();
        output = re_italic_under.replace_all(&output, "[i]$1[/i]").to_string();

        // 11. Strikethrough: ~~text~~
        let re_strike = Regex::new(r"~~(.+?)~~").unwrap();
        output = re_strike.replace_all(&output, "[s]$1[/s]").to_string();

        // 12. Smileys → Unicode emoji
        output = Self::convert_smileys(&output);

        (output, protected)
    }

    /// Convert emoji shortcodes (:smile:, :thumbsup:, etc.) to Unicode emoji
    /// using the full Unicode emoji database, plus common ASCII emoticons.
    fn convert_smileys(input: &str) -> String {
        // 1. Shortcode syntax :name: → Unicode emoji (full database, ~1800 emoji)
        let re_shortcode = Regex::new(r":([a-z0-9_+-]+):").unwrap();
        let mut output = re_shortcode
            .replace_all(input, |caps: &regex::Captures| {
                match emojis::get_by_shortcode(&caps[1]) {
                    Some(emoji) => emoji.as_str().to_string(),
                    None => caps[0].to_string(), // unknown shortcode: leave as-is
                }
            })
            .to_string();

        // 2. ASCII emoticons → Unicode (boundary-protected)
        let smileys: &[(&str, &str)] = &[
            (":'(", "😢"),
            ("O:)", "😇"),
            (">:(", "😠"),
            (":)", "🙂"),
            (":(", "🙁"),
            (":D", "😃"),
            (";)", "😉"),
            (":P", "😛"),
            (":p", "😛"),
            (":O", "😮"),
            (":o", "😮"),
            (":/", "😕"),
            (":|", "😐"),
            (":*", "😘"),
            ("B)", "😎"),
            ("XD", "😆"),
            ("xD", "😆"),
            ("<3", "❤️"),
        ];
        for &(smiley, emoji) in smileys {
            let escaped = regex::escape(smiley);
            let pattern = format!(r"(?m)(^|[\s]){}([\s,.!?]|$)", escaped);
            if let Ok(re) = Regex::new(&pattern) {
                output = re
                    .replace_all(&output, |caps: &regex::Captures| {
                        format!("{}{}{}", &caps[1], emoji, &caps[2])
                    })
                    .to_string();
            }
        }
        output
    }

    /// Convert BBCode to HTML
    pub fn to_html(input: &str) -> String {
        Self::render(input, &HtmlRenderer)
    }

    /// Convert BBCode to plain text
    pub fn to_plain(input: &str) -> String {
        Self::render(input, &PlainTextRenderer)
    }

    /// Convert BBCode to ANSI terminal output
    pub fn to_ansi(input: &str) -> String {
        Self::render(input, &AnsiRenderer)
    }

    fn process_simple_tag<F>(input: &str, tag: &str, renderer: F) -> String
    where
        F: Fn(&str) -> String,
    {
        let pattern = format!(r"\[{}\](.*?)\[/{}\]", tag, tag);
        let re = Regex::new(&pattern).unwrap();
        re.replace_all(input, |caps: &regex::Captures| renderer(&caps[1]))
            .to_string()
    }

    fn process_param_tag<F>(input: &str, tag: &str, renderer: F) -> String
    where
        F: Fn(&str, &str) -> String,
    {
        let pattern = format!(r"\[{}=([^\]]+)\](.*?)\[/{}\]", tag, tag);
        let re = Regex::new(&pattern).unwrap();
        re.replace_all(input, |caps: &regex::Captures| renderer(&caps[1], &caps[2]))
            .to_string()
    }
}

/// Strip all BBCode tags from text
pub fn strip_bbcode(input: &str) -> String {
    BBCodeParser::to_plain(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bold() {
        assert_eq!(
            BBCodeParser::to_html("[b]test[/b]"),
            "<strong>test</strong>"
        );
    }

    #[test]
    fn test_color() {
        assert_eq!(
            BBCodeParser::to_html("[color=red]test[/color]"),
            "<span style=\"color: red\">test</span>"
        );
    }

    #[test]
    fn test_strip() {
        assert_eq!(strip_bbcode("[b]hello[/b] [i]world[/i]"), "hello world");
    }

    #[test]
    fn italic_html() {
        assert_eq!(BBCodeParser::to_html("[i]x[/i]"), "<em>x</em>");
    }

    #[test]
    fn underline_html() {
        assert_eq!(BBCodeParser::to_html("[u]x[/u]"), "<u>x</u>");
    }

    #[test]
    fn strike_html() {
        assert_eq!(BBCodeParser::to_html("[s]x[/s]"), "<s>x</s>");
    }

    #[test]
    fn url_with_param() {
        assert_eq!(
            BBCodeParser::to_html("[url=https://ex.com]click[/url]"),
            "<a href=\"https://ex.com\">click</a>"
        );
    }

    #[test]
    fn url_simple() {
        assert_eq!(
            BBCodeParser::to_html("[url]https://ex.com[/url]"),
            "<a href=\"https://ex.com\">https://ex.com</a>"
        );
    }

    #[test]
    fn image_tag() {
        assert_eq!(
            BBCodeParser::to_html("[img]pic.png[/img]"),
            "<img src=\"pic.png\" />"
        );
    }

    #[test]
    fn nested_bold_italic() {
        // Inner tags processed first
        let result = BBCodeParser::to_html("[b][i]text[/i][/b]");
        assert_eq!(result, "<strong><em>text</em></strong>");
    }

    #[test]
    fn plain_text_url_different_text() {
        let result = BBCodeParser::to_plain("[url=https://ex.com]click[/url]");
        assert_eq!(result, "click (https://ex.com)");
    }

    #[test]
    fn plain_text_url_same_text() {
        let result = BBCodeParser::to_plain("[url]https://ex.com[/url]");
        assert_eq!(result, "https://ex.com");
    }

    #[test]
    fn plain_text_image() {
        assert_eq!(BBCodeParser::to_plain("[img]pic.png[/img]"), "[Image: pic.png]");
    }

    #[test]
    fn ansi_bold() {
        let result = BBCodeParser::to_ansi("[b]hi[/b]");
        assert_eq!(result, "\x1b[1mhi\x1b[0m");
    }

    #[test]
    fn ansi_color_red() {
        let result = BBCodeParser::to_ansi("[color=red]err[/color]");
        assert_eq!(result, "\x1b[31merr\x1b[0m");
    }

    #[test]
    fn ansi_unknown_color_uses_default() {
        let result = BBCodeParser::to_ansi("[color=#ff0000]x[/color]");
        assert_eq!(result, "\x1b[39mx\x1b[0m");
    }

    #[test]
    fn no_tags_unchanged() {
        assert_eq!(BBCodeParser::to_html("plain text"), "plain text");
        assert_eq!(BBCodeParser::to_plain("plain text"), "plain text");
    }

    #[test]
    fn malformed_tags_unchanged() {
        // Unclosed tags remain as-is
        assert_eq!(BBCodeParser::to_html("[b]open"), "[b]open");
    }

    #[test]
    fn size_html() {
        assert_eq!(
            BBCodeParser::to_html("[size=12]big[/size]"),
            "<span style=\"font-size: 12px\">big</span>"
        );
    }

    // --- Markdown support tests ---

    #[test]
    fn md_bold_html() {
        assert_eq!(
            BBCodeParser::to_html("**bold**"),
            "<strong>bold</strong>"
        );
    }

    #[test]
    fn md_bold_plain() {
        assert_eq!(BBCodeParser::to_plain("**bold**"), "bold");
    }

    #[test]
    fn md_bold_ansi() {
        assert_eq!(
            BBCodeParser::to_ansi("**bold**"),
            "\x1b[1mbold\x1b[0m"
        );
    }

    #[test]
    fn md_italic_html() {
        assert_eq!(
            BBCodeParser::to_html("*italic*"),
            "<em>italic</em>"
        );
    }

    #[test]
    fn md_italic_plain() {
        assert_eq!(BBCodeParser::to_plain("*italic*"), "italic");
    }

    #[test]
    fn md_italic_ansi() {
        assert_eq!(
            BBCodeParser::to_ansi("*italic*"),
            "\x1b[3mitalic\x1b[0m"
        );
    }

    #[test]
    fn md_strikethrough_html() {
        assert_eq!(
            BBCodeParser::to_html("~~struck~~"),
            "<s>struck</s>"
        );
    }

    #[test]
    fn md_strikethrough_plain() {
        assert_eq!(BBCodeParser::to_plain("~~struck~~"), "struck");
    }

    #[test]
    fn md_strikethrough_ansi() {
        assert_eq!(
            BBCodeParser::to_ansi("~~struck~~"),
            "\x1b[9mstruck\x1b[0m"
        );
    }

    #[test]
    fn md_inline_code_html() {
        assert_eq!(
            BBCodeParser::to_html("`code here`"),
            "<code>code here</code>"
        );
    }

    #[test]
    fn md_inline_code_plain() {
        assert_eq!(BBCodeParser::to_plain("`code here`"), "code here");
    }

    #[test]
    fn md_inline_code_ansi() {
        assert_eq!(
            BBCodeParser::to_ansi("`code here`"),
            "\x1b[7mcode here\x1b[0m"
        );
    }

    #[test]
    fn md_code_block_html() {
        assert_eq!(
            BBCodeParser::to_html("```\nhello\n```"),
            "<pre><code>hello\n</code></pre>"
        );
    }

    #[test]
    fn md_code_block_with_lang_html() {
        assert_eq!(
            BBCodeParser::to_html("```python\nprint(1)\n```"),
            "<pre><code>print(1)\n</code></pre>"
        );
    }

    #[test]
    fn md_code_block_ansi() {
        assert_eq!(
            BBCodeParser::to_ansi("```\nhello\n```"),
            "\x1b[2mhello\n\x1b[0m"
        );
    }

    #[test]
    fn md_code_protects_content() {
        // Markdown inside backticks should NOT be parsed
        assert_eq!(
            BBCodeParser::to_html("`**not bold**`"),
            "<code>**not bold**</code>"
        );
    }

    #[test]
    fn md_code_block_protects_content() {
        // BBCode inside fenced code should NOT be parsed
        assert_eq!(
            BBCodeParser::to_html("```\n[b]not bold[/b]\n```"),
            "<pre><code>[b]not bold[/b]\n</code></pre>"
        );
    }

    #[test]
    fn md_mixed_with_bbcode() {
        assert_eq!(
            BBCodeParser::to_html("[b]bb[/b] **md**"),
            "<strong>bb</strong> <strong>md</strong>"
        );
    }

    #[test]
    fn md_lone_asterisk_unchanged() {
        // A single * surrounded by spaces should not be converted
        assert_eq!(BBCodeParser::to_html("a * b"), "a * b");
    }

    #[test]
    fn md_bold_italic_combined() {
        // ***text*** → bold wraps italic
        assert_eq!(
            BBCodeParser::to_html("***text***"),
            "<strong><em>text</em></strong>"
        );
    }

    // --- Underscore variants ---

    #[test]
    fn md_underscore_italic_html() {
        assert_eq!(
            BBCodeParser::to_html("_italic_"),
            "<em>italic</em>"
        );
    }

    #[test]
    fn md_underscore_bold_html() {
        assert_eq!(
            BBCodeParser::to_html("__bold__"),
            "<strong>bold</strong>"
        );
    }

    #[test]
    fn md_underscore_bold_italic_html() {
        assert_eq!(
            BBCodeParser::to_html("___both___"),
            "<strong><em>both</em></strong>"
        );
    }

    #[test]
    fn md_underscore_in_identifier_unchanged() {
        // foo_bar_baz should NOT become italic
        assert_eq!(BBCodeParser::to_html("foo_bar_baz"), "foo_bar_baz");
    }

    #[test]
    fn md_underscore_italic_in_sentence() {
        assert_eq!(
            BBCodeParser::to_html("this is _important_ stuff"),
            "this is <em>important</em> stuff"
        );
    }

    // --- Checkboxes ---

    #[test]
    fn md_checkbox_checked() {
        assert_eq!(
            BBCodeParser::to_html("- [x] done"),
            "☑ done"
        );
    }

    #[test]
    fn md_checkbox_unchecked() {
        assert_eq!(
            BBCodeParser::to_html("- [ ] todo"),
            "☐ todo"
        );
    }

    #[test]
    fn md_checkbox_star() {
        assert_eq!(
            BBCodeParser::to_html("* [x] yes"),
            "☑ yes"
        );
    }

    // --- List items ---

    #[test]
    fn md_list_dash() {
        assert_eq!(
            BBCodeParser::to_html("- item one\n- item two"),
            "• item one\n• item two"
        );
    }

    #[test]
    fn md_list_star() {
        assert_eq!(
            BBCodeParser::to_html("* first\n* second"),
            "• first\n• second"
        );
    }

    #[test]
    fn md_list_indented() {
        assert_eq!(
            BBCodeParser::to_html("  - nested"),
            "  • nested"
        );
    }

    // --- Smileys ---

    #[test]
    fn smiley_smile() {
        assert_eq!(BBCodeParser::to_html("hello :)"), "hello 🙂");
    }

    #[test]
    fn smiley_sad() {
        assert_eq!(BBCodeParser::to_html("oh no :("), "oh no 🙁");
    }

    #[test]
    fn smiley_heart() {
        assert_eq!(BBCodeParser::to_html("love <3"), "love ❤\u{fe0f}");
    }

    #[test]
    fn smiley_grin() {
        assert_eq!(BBCodeParser::to_html("haha :D"), "haha 😃");
    }

    #[test]
    fn smiley_not_in_url() {
        // :/ inside a URL should NOT be converted
        let input = "see http://example.com";
        assert_eq!(BBCodeParser::to_html(input), input);
    }

    #[test]
    fn smiley_multiple() {
        assert_eq!(
            BBCodeParser::to_html(":) :D"),
            "🙂 😃"
        );
    }

    #[test]
    fn smiley_with_punctuation() {
        assert_eq!(
            BBCodeParser::to_html("nice :)!"),
            "nice 🙂!"
        );
    }

    #[test]
    fn smiley_cry() {
        assert_eq!(BBCodeParser::to_html("so sad :'("), "so sad 😢");
    }

    // --- Emoji shortcodes ---

    #[test]
    fn shortcode_smile() {
        assert_eq!(BBCodeParser::to_html(":smile:"), "😄");
    }

    #[test]
    fn shortcode_thumbsup() {
        assert_eq!(BBCodeParser::to_html(":thumbsup:"), "👍");
        assert_eq!(BBCodeParser::to_html(":+1:"), "👍");
    }

    #[test]
    fn shortcode_heart() {
        assert_eq!(BBCodeParser::to_html(":heart:"), "❤️");
    }

    #[test]
    fn shortcode_unknown_unchanged() {
        assert_eq!(BBCodeParser::to_html(":notanemoji:"), ":notanemoji:");
    }

    #[test]
    fn shortcode_in_sentence() {
        assert_eq!(
            BBCodeParser::to_html("great job :fire: keep going"),
            "great job 🔥 keep going"
        );
    }

    #[test]
    fn shortcode_multiple() {
        assert_eq!(
            BBCodeParser::to_html(":wave: hello :earth_americas:"),
            "👋 hello 🌎"
        );
    }

    // --- Markdown links ---

    #[test]
    fn md_link_html() {
        assert_eq!(
            BBCodeParser::to_html("[link](http://example.com)"),
            "<a href=\"http://example.com\">link</a>"
        );
    }

    #[test]
    fn md_link_https() {
        assert_eq!(
            BBCodeParser::to_html("[click](https://example.com/path?q=1)"),
            "<a href=\"https://example.com/path?q=1\">click</a>"
        );
    }

    #[test]
    fn md_link_in_bold() {
        assert_eq!(
            BBCodeParser::to_html("**[click](http://example.com)**"),
            "<strong><a href=\"http://example.com\">click</a></strong>"
        );
    }

    #[test]
    fn md_link_no_protocol_unchanged() {
        // Relative URLs without http(s):// are left as-is
        assert_eq!(
            BBCodeParser::to_html("[text](relative/path)"),
            "[text](relative/path)"
        );
    }

    // --- Markdown images ---

    #[test]
    fn md_image_html() {
        assert_eq!(
            BBCodeParser::to_html("![img](pic.png)"),
            "<img src=\"pic.png\" />"
        );
    }

    #[test]
    fn md_image_with_title() {
        assert_eq!(
            BBCodeParser::to_html("![alt](pic.png \"my title\")"),
            "<img src=\"pic.png\" />"
        );
    }

    #[test]
    fn md_image_does_not_match_link() {
        // A normal link should NOT become an image
        let result = BBCodeParser::to_html("[link](http://example.com)");
        assert!(!result.contains("<img"));
        assert!(result.contains("<a href"));
    }

    #[test]
    fn md_link_does_not_match_image() {
        // An image should NOT become a link
        let result = BBCodeParser::to_html("![photo](http://example.com/pic.png)");
        assert!(result.contains("<img"));
        assert!(!result.contains("<a href"));
    }

    // --- Markdown tables ---

    #[test]
    fn md_table_simple() {
        let input = "| A | B |\n| --- | --- |\n| 1 | 2 |";
        assert_eq!(
            BBCodeParser::to_html(input),
            "<pre><code>| A | B |\n| 1 | 2 |</code></pre>"
        );
    }

    #[test]
    fn md_table_with_alignment() {
        let input = "| Left | Center | Right |\n| :--- | :---: | ---: |\n| a | b | c |";
        assert_eq!(
            BBCodeParser::to_html(input),
            "<pre><code>| Left | Center | Right |\n| a | b | c |</code></pre>"
        );
    }

    #[test]
    fn md_table_with_surrounding_text() {
        let input = "Before\n| A | B |\n| --- | --- |\n| 1 | 2 |\nAfter";
        assert_eq!(
            BBCodeParser::to_html(input),
            "Before\n<pre><code>| A | B |\n| 1 | 2 |</code></pre>\nAfter"
        );
    }

    #[test]
    fn md_table_single_pipe_line_not_table() {
        // A single line starting with | should not be treated as a table
        assert_eq!(
            BBCodeParser::to_html("| just a pipe"),
            "| just a pipe"
        );
    }

    // --- Ordered lists (already working) ---

    #[test]
    fn md_ordered_list_with_sub_bullets() {
        let input = "1. First\n   - sub\n2. Second";
        assert_eq!(
            BBCodeParser::to_html(input),
            "1. First\n   • sub\n2. Second"
        );
    }
}
