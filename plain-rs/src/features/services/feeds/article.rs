use super::inner_html;
use plain_libs::utils::html_to_markdown::dom::{Dom, Node};

#[derive(Default)]
struct Content {
    text: usize,
    links: usize,
    score: usize,
    media: bool,
}

pub(super) fn article_html(html: &str) -> String {
    let mut dom = Dom::parse(html);
    let mut ignored = vec![false; dom.0.len()];
    for id in 1..dom.0.len() {
        let node = &dom.0[id];
        ignored[id] = node.parent.is_some_and(|p| ignored[p]) || page_chrome(node);
    }
    for node in &mut dom.0 {
        node.children.retain(|&id| !ignored[id]);
    }

    let mut content: Vec<_> = (0..dom.0.len()).map(|_| Content::default()).collect();
    let mut best = None;
    for id in (1..dom.0.len()).rev().filter(|&id| !ignored[id]) {
        let node = &dom.0[id];
        let mut stats = Content {
            text: node.text.chars().filter(|c| !c.is_whitespace()).count(),
            media: matches!(node.tag.as_str(), "img" | "video" | "audio" | "iframe"),
            ..Content::default()
        };
        for &child in &node.children {
            let child = &content[child];
            stats.text += child.text;
            stats.links += child.links;
            stats.score += child.score;
            stats.media |= child.media;
        }
        if node.tag == "a" {
            stats.links = stats.text;
        }
        let prose = stats.text.saturating_sub(stats.links);
        let semantic = !matches!(node.tag.as_str(), "html" | "body")
            && (matches!(node.tag.as_str(), "article" | "main")
                || node.attr("role") == "main"
                || node
                    .attr("itemprop")
                    .split_ascii_whitespace()
                    .any(|name| name == "articleBody"));
        if matches!(node.tag.as_str(), "p" | "pre") && prose >= 25 {
            stats.score = prose;
        } else if !node.tag.is_empty() {
            // Prefer the nearest container with the prose, not its page-level wrappers.
            stats.score = stats.score * 4 / 5;
        }
        if node.tag == "div"
            && prose >= 50
            && node.children.iter().all(|&child| !dom.0[child].block())
        {
            stats.score = prose;
        }
        let score = if semantic && (prose > 0 || stats.media) {
            stats.score.max(prose.min(25)) + 16
        } else if matches!(node.tag.as_str(), "div" | "section") && prose >= 50 && stats.score >= 40
        {
            stats.score
        } else {
            0
        };
        let score = score * prose.max(1) / stats.text.max(1);
        if score > 0 && best.is_none_or(|(previous, _)| score > previous) {
            best = Some((score, id));
        }
        content[id] = stats;
    }
    best.map_or_else(String::new, |(_, id)| inner_html(&dom, id))
}

fn page_chrome(node: &Node) -> bool {
    matches!(
        node.tag.as_str(),
        "head" | "script" | "style" | "template" | "header" | "footer" | "nav" | "aside" | "form"
    ) || matches!(
        node.attr("role"),
        "navigation" | "banner" | "complementary" | "contentinfo"
    ) || node
        .attrs
        .iter()
        .any(|(name, value)| name == "hidden" || (name == "aria-hidden" && value == "true"))
        || [node.attr("id"), node.attr("class")]
            .into_iter()
            .flat_map(str::split_ascii_whitespace)
            .any(|token| {
                matches!(
                    token.to_ascii_lowercase().as_str(),
                    "ad" | "ads"
                        | "advertisement"
                        | "adincontent"
                        | "adeditor"
                        | "sidebar"
                        | "related"
                        | "related-posts"
                        | "comments"
                        | "social-share"
                )
            })
}
