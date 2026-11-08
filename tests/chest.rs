use survival::items::ItemId;
use survival::objects::chest::parse_content;

#[test]
fn content_lists_items_with_counts() {
    assert_eq!(
        parse_content(" apple*3, healing_potion ,sword * 2"),
        vec![
            (ItemId::from("apple"), 3),
            (ItemId::from("healing_potion"), 1),
            (ItemId::from("sword"), 2),
        ]
    );
}

#[test]
fn content_skips_what_does_not_parse() {
    assert_eq!(
        parse_content("apple*x, , potion*2"),
        vec![(ItemId::from("potion"), 2)]
    );
    assert!(parse_content("").is_empty());
}
