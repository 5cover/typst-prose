#show "ACME": "Acme Corporation"
#show "TODO": it => upper(it.text)

The report by ACME covers the TODO items.

#let note(body) = block[Note: #body]
#note[Generated wrapper text.]
