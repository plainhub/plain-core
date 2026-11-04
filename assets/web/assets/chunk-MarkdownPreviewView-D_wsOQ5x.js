import{i as e}from"./chunk-rolldown-runtime-aKtaBQYM.js";import{B as t,b as n,d as r,m as i,t as a,u as o}from"./chunk-_plugin-vue_export-helper-DhhQ8qWg.js";import{t as s}from"./chunk-katex-zLIKX-We.js";import{a as c,i as l,n as u,o as d,r as f,s as p,t as m}from"./chunk-markdown-task-lists-Uj_ktKaN.js";var h=e(c(),1),g=e(l(),1),_=e(f(),1),v=e(u(),1),y={class:`md-preview-page`},b={class:`md-preview-main`},x=[`innerHTML`],S=`# Heading 1

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
`,C=a(n({__name:`MarkdownPreviewView`,setup(e){let n=new p({html:!0,xhtmlOut:!0,breaks:!0,linkify:!0,typographer:!0}).use(d).use(h.default).use(g.default).use(_.default).use(v.default,{engine:s,delimiters:`dollars`,katexOptions:{output:`html`}}).use(m),a=o(()=>n.render(S));return(e,n)=>(t(),i(`div`,y,[n[0]||=r(`header`,{class:`md-preview-header`},[r(`h1`,null,`Markdown Preview`)],-1),r(`main`,b,[r(`div`,{class:`md-container`,innerHTML:a.value},null,8,x)])]))}}),[[`__scopeId`,`data-v-ec83230d`]]);export{C as default};