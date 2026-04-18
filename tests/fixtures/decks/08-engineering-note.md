<!-- fixture: one cell containing multiple blocks (heading + prose + list + code, no blank lines between) -->

# Parser Usage
	Parse a deck from a Markdown source string, then walk slides.
	- call `parse_deck(source)` once per load
	- iterate `deck.slides` for render
	- forward keys to the navigator
```rust
let deck = parse_deck(&source)?;
for slide in &deck.slides {
    render_slide(slide);
}
```
