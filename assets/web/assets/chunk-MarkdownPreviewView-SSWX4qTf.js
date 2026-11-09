import{b as e,d as t,m as n,t as r,u as i,z as a}from"./chunk-_plugin-vue_export-helper-BqdUuCM-.js";import{t as o}from"./chunk-markdown-renderer-BfJlJCR9.js";var s={class:`md-preview-page`},c={class:`md-preview-main`},l=[`innerHTML`],u=`# Heading 1

## Heading 2

### Heading 3

#### Heading 4

This is a regular paragraph with some content. It demonstrates the unified markdown reading theme. The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

---

## Text Formatting

You can use **bold text**, *italic text*, and ***both***. There's also ~~strikethrough~~ and \`inline code\` formatting. superscript: x^2^, subscript: H~2~O, ++inserted text++, and ==marked text==.

## Lists

### Unordered List
- First item
- Second item
  - Nested item
  - Another nested item
- Third item

### Ordered List
1. First step
2. Second step
3. Third step

### Task List
- [x] Completed task
- [x] Another completed task
- [ ] Pending task

## Code Blocks

\`\`\`kotlin
fun main() {
    val greeting = "Hello, Markdown!"
    println(greeting)
}
\`\`\`

\`\`\`javascript
function fibonacci(n) {
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}
\`\`\`

## Blockquotes

> "This is a blockquote. It's often used to highlight important information or quotes from other sources."
> - Source attribution

> Nested blockquote with multiple lines.
> Each line starts with a > character.

## Tables

| Feature | Description |
|---------|-------------|
| Headings | Clear hierarchy with proper spacing |
| Code | Dark background with monospace font |
| Quote | Muted text with left accent border |
| Table | Clean borders, comfortable rows with rounded corners |

## Links

[Visit PlainApp](https://plainapp.app) for more information.

## Images

![Placeholder Image](https://picsum.photos/600/300)

## Math

Inline math: $E = mc^2$

Block math:
$$
\\int_0^1 x^2 dx = \\frac{1}{3}
$$

## Definition List

Term 1
: Definition for term 1

Term 2
: Definition for term 2

## Footnotes

Here is a footnote reference[^1].

[^1]: This is the footnote content.
`,d=r(e({__name:`MarkdownPreviewView`,setup(e){let r=o(!0),d=i(()=>r.render(u));return(e,r)=>(a(),n(`div`,s,[r[0]||=t(`header`,{class:`md-preview-header`},[t(`h1`,null,`Markdown Preview`)],-1),t(`main`,c,[t(`div`,{class:`md-container`,innerHTML:d.value},null,8,l)])]))}}),[[`__scopeId`,`data-v-d08d89c1`]]);export{d as default};