//! Components unit tests

use ratatui::layout::Rect;
use ratatui::prelude::Rect as PreludeRect;
use qinAegis_lib::tui::components::three_panel;

fn make_rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
    Rect::new(x, y, w, h)
}

#[test]
fn test_three_panel_splits_correctly_at_24() {
    let area = make_rect(0, 0, 80, 24);
    let [top, middle, bottom] = three_panel(area);

    assert_eq!(top.height, 3, "top should be 3 rows");
    assert_eq!(bottom.height, 3, "bottom should be 3 rows");
    assert_eq!(top.width, 80);
    assert_eq!(middle.width, 80);
    assert_eq!(bottom.width, 80);
    assert_eq!(top.x, 0);
    assert_eq!(top.y, 0);
    assert_eq!(bottom.x, 0);
    assert_eq!(bottom.y, 21); // 24 - 3
}

#[test]
fn test_three_panel_small_height() {
    let area = make_rect(0, 0, 80, 5);
    let [top, middle, bottom] = three_panel(area);

    // Each panel gets at least 1 row
    assert!(top.height >= 1);
    assert!(middle.height >= 1);
    assert!(bottom.height >= 1);
}

#[test]
fn test_three_panel_exact_height_7() {
    let area = make_rect(0, 0, 80, 7);
    let [top, middle, bottom] = three_panel(area);

    // 3 + remaining + 3
    assert_eq!(top.height, 3);
    assert_eq!(bottom.height, 3);
    assert_eq!(middle.height, 1); // 7 - 3 - 3 = 1
}

#[test]
fn test_three_panel_preserves_width() {
    let area = make_rect(10, 5, 50, 24);
    let [top, middle, bottom] = three_panel(area);

    assert_eq!(top.width, 50);
    assert_eq!(middle.width, 50);
    assert_eq!(bottom.width, 50);
    assert_eq!(top.x, 10);
    assert_eq!(top.y, 5);
}