// Run: npm test. Checks highlightYaml token classes and that it never changes the text.
import { highlightYaml } from "../src/lib/highlight.ts";
const y = `# workflow
name: "my wf" # trailing
jobs:
  - name: a
    command: |
      echo "a: b" # not a comment
      done
    depends_on: [a, "b"]
    num_cpus: 4
    enabled: true
  - b
url: http://x:80/p
key with space: 1.5e3
ref: *anchor
`;
const h = highlightYaml(y);

const strip = h.replace(/<[^>]+>/g, "").replace(/&lt;/g,"<").replace(/&gt;/g,">").replace(/&amp;/g,"&");
if (strip !== y) { console.error("TEXT CHANGED"); process.exit(1); }
const must = ['<span class="c"># workflow</span>', '<span class="k">name</span>: <span class="s">"my wf"</span> <span class="c"># trailing</span>',
 '<span class="s">      echo "a: b" # not a comment</span>', '<span class="n">4</span>', '<span class="b">true</span>',
 '<span class="k">url</span>: <span class="s">http://x:80/p</span>', '<span class="k">key with space</span>', '<span class="p">*anchor</span>',
 '<span class="p">- </span><span class="k">name</span>'];
for (const m of must) if (!h.includes(m)) { console.error("MISSING", m); process.exit(1); }
console.log("OK");
