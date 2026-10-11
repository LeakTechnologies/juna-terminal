use super::*;

const AREA: Rect = Rect::new(0, 0, 1000, 600);
const MIN: MinSize = MinSize {
    width: 40,
    height: 20,
};

fn p(n: u32) -> PaneId {
    PaneId(n)
}

fn by_id(report: &LayoutReport, id: PaneId) -> Rect {
    report
        .panes
        .iter()
        .find(|x| x.pane == id)
        .unwrap_or_else(|| panic!("pane {id:?} missing from layout"))
        .rect
}

fn assert_invariants(tree: &Tree, area: Rect, min: MinSize) {
    let report = tree.layout(area, min);
    let panes = &report.panes;
    assert_eq!(
        panes.len(),
        tree.pane_count(),
        "layout emitted {} rects for {} panes",
        panes.len(),
        tree.pane_count()
    );

    if tree.is_empty() {
        assert!(panes.is_empty(), "an empty tree must emit no rects");
        return;
    }

    let total: i128 = panes.iter().map(|x| x.rect.area()).sum();
    assert_eq!(
        total,
        area.area(),
        "leaf areas must sum to the container area"
    );

    for a in panes {
        assert!(area.contains(&a.rect), "{:?} escapes {area:?}", a.pane);
        let really_below = a.rect.w < min.width || a.rect.h < min.height;
        assert_eq!(
            a.below_minimum, really_below,
            "{:?} reports below_minimum={} but rect is {:?}",
            a.pane, a.below_minimum, a.rect
        );
    }

    for i in 0..panes.len() {
        for j in (i + 1)..panes.len() {
            assert!(
                !panes[i].rect.overlaps(&panes[j].rect),
                "{:?} overlaps {:?}",
                panes[i].pane,
                panes[j].pane
            );
        }
    }
}

#[test]
fn empty_tree_lays_out_nothing() {
    let t = Tree::new();
    let r = t.layout(AREA, MIN);
    assert!(r.panes.is_empty());
    assert!(r.unsatisfied.is_empty());
}

#[test]
fn single_pane_fills_the_container() {
    let t = Tree::with_single(p(1));
    let r = t.layout(AREA, MIN);
    assert_eq!(by_id(&r, p(1)), AREA);
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn horizontal_split_divides_width_only() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    let r = t.layout(AREA, MIN);
    let a = by_id(&r, p(1));
    let b = by_id(&r, p(2));
    assert_eq!(a.w, 500);
    assert_eq!(b.w, 500);
    assert_eq!(a.h, AREA.h);
    assert_eq!(b.h, AREA.h);
    assert_eq!(b.x, a.right() as i32);
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn vertical_split_divides_height_only() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Vertical, Ratio::HALF);
    let r = t.layout(AREA, MIN);
    let a = by_id(&r, p(1));
    let b = by_id(&r, p(2));
    assert_eq!(a.h, 300);
    assert_eq!(b.h, 300);
    assert_eq!(a.w, AREA.w);
    assert_eq!(b.y, a.bottom() as i32);
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn unequal_ratio_is_respected() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::new(300).unwrap());
    let r = t.layout(AREA, MIN);
    assert_eq!(by_id(&r, p(1)).w, 300);
    assert_eq!(by_id(&r, p(2)).w, 700);
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn nested_splits_produce_a_quadtree_without_gaps() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    t.set_focus(p(1));
    t.split_focused(Axis::Vertical, Ratio::HALF);
    t.set_focus(p(3));
    t.split_focused(Axis::Horizontal, Ratio::HALF);

    assert_eq!(t.pane_count(), 4);
    let r = t.layout(AREA, MIN);
    assert_invariants(&t, AREA, MIN);
    assert_eq!(by_id(&r, p(1)).w, 500);
    assert_eq!(by_id(&r, p(1)).h, 300);
    assert_eq!(by_id(&r, p(3)).w, 250);
}

#[test]
fn minimum_size_is_enforced_when_the_container_allows() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::new(10).unwrap());
    let r = t.layout(AREA, MIN);
    let a = by_id(&r, p(1));
    let b = by_id(&r, p(2));
    assert!(a.w >= MIN.width, "first pane {a:?} below minimum");
    assert!(b.w >= MIN.width, "second pane {b:?} below minimum");
    assert_eq!(a.w + b.w, AREA.w);
    assert!(r.unsatisfied.is_empty());
}

#[test]
fn unsatisfiable_minimum_is_reported_not_hidden() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    let tiny = Rect::new(0, 0, 20, 10);
    let r = t.layout(tiny, MIN);
    assert_eq!(r.unsatisfied.len(), 2, "both panes must be reported");
    assert_eq!(r.panes.iter().filter(|x| x.below_minimum).count(), 2);
    let total: i128 = r.panes.iter().map(|x| x.rect.area()).sum();
    assert_eq!(total, tiny.area(), "area still conserved when min is unmet");
}

#[test]
fn closing_a_pane_collapses_its_parent() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    assert_eq!(t.pane_count(), 2);
    assert!(t.close(p(2)));
    assert_eq!(t.pane_count(), 1);
    assert!(!t.contains(p(2)));
    let r = t.layout(AREA, MIN);
    assert_eq!(by_id(&r, p(1)), AREA, "survivor inherits the whole area");
}

#[test]
fn closing_the_root_pane_empties_the_tree() {
    let mut t = Tree::with_single(p(1));
    assert!(t.close(p(1)));
    assert!(t.is_empty());
    assert_eq!(t.focus(), None);
    assert!(t.layout(AREA, MIN).panes.is_empty());
    assert_eq!(t.split_focused(Axis::Horizontal, Ratio::HALF), None);
}

#[test]
fn closing_a_deep_pane_promotes_its_sibling() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    t.set_focus(p(1));
    t.split_focused(Axis::Vertical, Ratio::HALF);
    assert_eq!(t.panes(), vec![p(1), p(3), p(2)]);

    assert!(t.close(p(3)));
    assert_eq!(t.panes(), vec![p(1), p(2)]);
    let r = t.layout(AREA, MIN);
    assert_eq!(
        by_id(&r, p(1)),
        Rect::new(0, 0, 500, 600),
        "survivor inherits the collapsed subtree's rect"
    );
    assert_eq!(
        by_id(&r, p(2)),
        Rect::new(500, 0, 500, 600),
        "unrelated pane is untouched"
    );
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn closing_an_unknown_pane_is_rejected() {
    let mut t = Tree::with_single(p(1));
    assert!(!t.close(p(99)));
    assert_eq!(t.pane_count(), 1);
}

#[test]
fn closing_the_focused_pane_moves_focus_to_a_survivor() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    assert_eq!(t.focus(), Some(p(2)));
    t.close(p(2));
    assert_eq!(t.focus(), Some(p(1)));
}

#[test]
fn focus_moves_in_each_direction() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    t.set_focus(p(2));
    t.split_focused(Axis::Vertical, Ratio::HALF);

    assert_eq!(t.pane_count(), 3);
    assert_eq!(t.move_focus(Direction::Left), Some(p(1)));
    assert_eq!(t.focus(), Some(p(1)));
    assert_eq!(t.move_focus(Direction::Right), Some(p(2)));
    assert_eq!(t.move_focus(Direction::Down), Some(p(3)));
    assert_eq!(t.move_focus(Direction::Up), Some(p(2)));
}

#[test]
fn focus_movement_stops_at_edges() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::HALF);
    t.set_focus(p(1));
    assert_eq!(t.move_focus(Direction::Left), None);
    assert_eq!(t.move_focus(Direction::Up), None);
    assert_eq!(t.focus(), Some(p(1)), "focus must not drift on a miss");
    assert_eq!(t.move_focus(Direction::Right), Some(p(2)));
}

#[test]
fn focus_movement_from_an_unknown_pane_is_none() {
    let t = Tree::with_single(p(1));
    assert_eq!(t.pane_in_direction(p(99), Direction::Left), None);
}

#[test]
fn layout_is_deterministic_across_repeated_runs() {
    let mut t = Tree::with_single(p(1));
    for i in 0..12 {
        t.set_focus(PaneId(1 + i % 3));
        t.split_focused(
            if i % 2 == 0 {
                Axis::Horizontal
            } else {
                Axis::Vertical
            },
            Ratio::new((250 + i * 30) as u16).unwrap(),
        );
    }
    let first = t.layout(AREA, MIN);
    for _ in 0..32 {
        assert_eq!(t.layout(AREA, MIN), first, "layout is not deterministic");
    }
    assert_invariants(&t, AREA, MIN);
}

#[test]
fn ratio_rejects_degenerate_values() {
    assert!(Ratio::new(0).is_none());
    assert!(Ratio::new(1000).is_none());
    assert!(Ratio::new(1).is_some());
    assert!(Ratio::new(999).is_some());
}

#[test]
fn invariants_hold_across_a_seeded_sweep() {
    let mut seed = 0x5eed_1234_9abc_def0u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    for trial in 0..200 {
        let mut t = Tree::with_single(p(1));
        let splits = (next() % 11) as usize;
        for _ in 0..splits {
            t.set_focus(PaneId(1 + (next() % (t.pane_count() as u64 + 1)) as u32));
            let axis = if next() % 2 == 0 {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
            let permille = 1 + (next() % 999) as u16;
            t.split_focused(axis, Ratio::new(permille).unwrap());
        }

        let w = 1 + (next() % 4000) as i32;
        let h = 1 + (next() % 3000) as i32;
        let area = Rect::new((next() % 500) as i32, (next() % 500) as i32, w as u32, h as u32);
        let min = MinSize {
            width: (next() % 200) as u32,
            height: (next() % 200) as u32,
        };

        assert_invariants(&t, area, min);

        let panes = t.panes();
        if !panes.is_empty() {
            let victim = panes[(next() % panes.len() as u64) as usize];
            let before = t.pane_count();
            assert!(t.close(victim));
            assert_eq!(t.pane_count(), before - 1);
            assert!(!t.contains(victim));
            assert_invariants(&t, area, min);
        }

        if trial % 40 == 0 {
            println!("trial {trial} panes={} ok", t.pane_count());
        }
    }
}

#[test]
fn extreme_containers_do_not_panic() {
    let mut t = Tree::with_single(p(1));
    t.split_focused(Axis::Horizontal, Ratio::new(1).unwrap());
    t.set_focus(p(1));
    t.split_focused(Axis::Vertical, Ratio::new(999).unwrap());

    for area in [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 1, 0),
        Rect::new(0, 0, u32::MAX, 1),
        Rect::new(0, 0, 1, u32::MAX),
        Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX),
        Rect::new(i32::MAX, i32::MAX, 0, 0),
    ] {
        let r = t.layout(area, MinSize::default());
        let total: i128 = r.panes.iter().map(|x| x.rect.area()).sum();
        assert_eq!(total, area.area(), "area not conserved for {area:?}");
    }
}
