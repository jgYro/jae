# Just Another Editor

Kind of bored of emacs, vim and helix and general the paradigms of modal editing. I really love the idea of emacs, but at this point there's too many features from the decades of its existence I am pretty indifferent about so I decided to take a lot of inspiration from it with this editor.

## Key Ideas
- Non-modal editing
- Simple yet powerful interface
- "Operate" mode which will basically be like https://cribl.io in a box when operating on text
- Advanced binary analysis tooling built-in
- Unix based principles leveraging external tools and STDIN and STDOUT where possible

## Hard Stances
- It is expected to use a terminal multiplexer like tmux or zellij
- There will be no direct git support outside of gutter type rendering
- - I use a custom tmux binding for a floating window with lazygit to handle majority of that
- Similarly, there will be no directly integrate terminal

## To be done
- [ ] LSP
- [ ] Tree-sitter
- [ ] Multiple Buffers (not sure how I want to do this yet)
- [ ] Opening files via arguments (like ```j text.txt```)
- [ ] Jump to character
- [ ] Multiple cursors
- [ ] Content specific rendering (like a spreadsheet viewer for csv or markdown parsing)
- [ ] Mind map / wiki / note system

## Tips & Tricks
- Use "j" as alias to the binary