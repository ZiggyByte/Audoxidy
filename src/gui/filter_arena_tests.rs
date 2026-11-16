//! Row identity and reuse of the filter arena.
//!
//! The tests build a filter index by hand and read the resolved tree back, so
//! they exercise the arena's bookkeeping without a widget tree or a database.
//! They pin that a path resolves only on demand, that a search keeps the
//! matching branch and its ancestors, and that a subfilter keeps the whole
//! matching subtree.

use super::*;

fn manager_with_index() -> LibraryFiltersManager {
    let mut nodes = Vec::new();
    // Rock -> Queen -> Innuendo
    nodes.push(FilterNode {
        label: std::sync::Arc::from("Rock"),
        parent: NO_PARENT,
        children: vec![1],
        depth: 0,
        filter: FilterType::Genre,
    });
    nodes.push(FilterNode {
        label: std::sync::Arc::from("Queen"),
        parent: 0,
        children: vec![2],
        depth: 1,
        filter: FilterType::Genre,
    });
    nodes.push(FilterNode {
        label: std::sync::Arc::from("Innuendo"),
        parent: 1,
        children: Vec::new(),
        depth: 2,
        filter: FilterType::Genre,
    });
    // Jazz -> Miles
    nodes.push(FilterNode {
        label: std::sync::Arc::from("Jazz"),
        parent: NO_PARENT,
        children: vec![4],
        depth: 0,
        filter: FilterType::Genre,
    });
    nodes.push(FilterNode {
        label: std::sync::Arc::from("Miles"),
        parent: 3,
        children: Vec::new(),
        depth: 1,
        filter: FilterType::Genre,
    });

    let mut roots = HashMap::new();
    roots.insert(FilterType::Genre, vec![0, 3]);
    let mut subfilters = HashMap::new();
    subfilters.insert(FilterType::Genre, vec!["J".to_string(), "R".to_string()]);

    let mut manager = LibraryFiltersManager::default();
    manager.filter_index = Some(FilterIndex {
        nodes,
        roots,
        subfilters,
    });
    manager.current_filter = FilterType::Genre;
    manager.apply_view();
    manager
}

fn visible_ids(manager: &LibraryFiltersManager) -> Vec<FilterNodeId> {
    manager
        .get_visible_tree_items()
        .2
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[test]
fn test_filter_arena_resolves_the_path_only_on_demand() {
    let manager = manager_with_index();
    let (filter, labels) = manager.node_levels(2).expect("el nodo existe");

    assert_eq!(filter, FilterType::Genre);
    assert_eq!(labels, vec!["Rock", "Queen", "Innuendo"]);
}

#[test]
fn test_filter_search_keeps_the_matching_branch_and_its_parents() {
    let mut manager = manager_with_index();
    manager.search_query = "innu".to_string();
    manager.apply_view();

    assert_eq!(
        manager.tree_data,
        vec![0],
        "solo el árbol con coincidencias"
    );
    assert_eq!(
        visible_ids(&manager),
        vec![0],
        "sin expandir solo se ve la raíz"
    );

    manager.expanded_nodes.insert(0);
    manager.refresh_flattened_tree();
    assert_eq!(
        visible_ids(&manager),
        vec![0, 1],
        "el padre de la coincidencia"
    );

    manager.expanded_nodes.insert(1);
    manager.refresh_flattened_tree();
    assert_eq!(visible_ids(&manager), vec![0, 1, 2]);
}

#[test]
fn test_filter_search_matching_a_root_keeps_all_its_descendants() {
    let mut manager = manager_with_index();
    manager.search_query = "rock".to_string();
    manager.expanded_nodes.insert(0);
    manager.expanded_nodes.insert(1);
    manager.apply_view();
    manager.refresh_flattened_tree();

    assert_eq!(visible_ids(&manager), vec![0, 1, 2]);
    assert!(manager.tree_data.contains(&0));
}

#[test]
fn test_filter_subfilter_keeps_the_whole_matching_subtree() {
    let mut manager = manager_with_index();
    manager.selected_subfilter = Some("R".to_string());
    manager.expanded_nodes.insert(0);
    manager.expanded_nodes.insert(1);
    manager.apply_view();
    manager.refresh_flattened_tree();

    assert_eq!(manager.tree_data, vec![0], "solo la raíz de la letra R");
    assert_eq!(visible_ids(&manager), vec![0, 1, 2]);
}
