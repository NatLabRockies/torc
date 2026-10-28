-- Adapt JOSS author metadata to Pandoc's standard LaTeX title block.
function Meta(meta)
  local authors = pandoc.List()
  local corresponding = false
  for i, author in ipairs(meta.authors) do
    if i > 1 then
      authors:insert(i % 2 == 1 and pandoc.LineBreak() or pandoc.Str(", "))
    end
    authors:extend(author.name)
    local affiliation = pandoc.utils.stringify(author.affiliation)
    if author.corresponding then
      affiliation = affiliation .. ",*"
      corresponding = true
    end
    authors:insert(pandoc.Superscript({pandoc.Str(affiliation)}))
  end
  for _, affiliation in ipairs(meta.affiliations) do
    authors:insert(pandoc.LineBreak())
    authors:insert(pandoc.Superscript({pandoc.Str(pandoc.utils.stringify(affiliation.index))}))
    authors:extend(affiliation.name)
  end
  if corresponding then
    authors:insert(pandoc.LineBreak())
    authors:insert(pandoc.Str("* Corresponding author"))
  end
  meta.author = pandoc.MetaInlines(authors)
  return meta
end
