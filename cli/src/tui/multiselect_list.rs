use super::navigable_list::{Keyed, NavigableList};
use ratatui::widgets::ListState;
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub enum SelectionMode {
    None,
    Select,
    Deselect,
}

pub struct MultiselectList<Item>
where
    Item: Keyed,
{
    // Represents the location of the cursor in the list
    state: ListState,

    // Represents the items in the list
    items: Vec<Item>,

    // Holds the keys of the selected items
    selected_items: HashSet<u64>,

    // Represents whether moving the cursor selects or deselects items
    selection_mode: SelectionMode,
}

impl<Item> NavigableList<Item> for MultiselectList<Item>
where
    Item: Keyed,
{
    fn get_items(&self) -> &Vec<Item> {
        &self.items
    }

    fn get_items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    fn get_cursor(&self) -> Option<usize> {
        self.state.selected()
    }

    // Move the cursor to the new location, potentially selecting or deselecting items based on selection_mode
    fn set_cursor(&mut self, new_cursor: Option<usize>) {
        let old_cursor = self.state.selected();
        self.state.select(new_cursor);
        if let Some(select) = match self.selection_mode {
            SelectionMode::None => None,
            SelectionMode::Select => Some(true),
            SelectionMode::Deselect => Some(false),
        } {
            let touched_indexes: Vec<usize> = match (old_cursor, new_cursor) {
                (None, Some(index)) => (0..=index).collect(),
                (Some(start), Some(end)) => {
                    (std::cmp::min(start, end)..=std::cmp::max(start, end)).collect()
                }
                _ => vec![],
            };
            for index in touched_indexes {
                if let Some(item) = self.items.get(index) {
                    self.set_item_selected(item.get_key(), select);
                }
            }
        }
    }
}

impl<Item> MultiselectList<Item>
where
    Item: Keyed,
{
    // Create a new list with no items
    pub fn new() -> Self {
        MultiselectList {
            state: ListState::default(),
            items: vec![],
            selected_items: HashSet::new(),
            selection_mode: SelectionMode::None,
        }
    }

    // Return a reference to the list state
    pub fn get_list_state(&mut self) -> &mut ListState {
        &mut self.state
    }

    // Get the current selection mode
    pub fn get_selection_mode(&self) -> SelectionMode {
        self.selection_mode
    }

    // Set the current selection mode
    pub fn set_selection_mode(&mut self, selection_mode: SelectionMode) {
        self.selection_mode = selection_mode;
    }

    // Iterate over the items and their selected state
    pub fn iter_items_with_selected(&self) -> impl ExactSizeIterator<Item = (&Item, bool)> {
        self.items
            .iter()
            .map(|item| (item, self.selected_items.contains(&item.get_key())))
    }

    // Replace the list's items with a new set of items
    pub fn replace_items(&mut self, items: Vec<Item>) {
        // Temporarily disable the the selection mode so that replace_items
        // doesn't select or deselect items when it moves the cursor
        let selection_mode = self.selection_mode;
        self.selection_mode = SelectionMode::None;

        // Remember which items were selected
        let old_selected_items = self.selected_items.clone();

        NavigableList::replace_items(self, items);

        // Restore the selected state of previously selected items
        self.selected_items = self
            .items
            .iter()
            .map(Keyed::get_key)
            .filter(|id| old_selected_items.contains(id))
            .collect();

        // Restore the selection mode
        self.selection_mode = selection_mode;
    }

    // Determine whether an item is selected by its key
    pub fn get_item_selected(&self, key: u64) -> bool {
        self.selected_items.contains(&key)
    }

    // Set whether an item is selected by its key
    pub fn set_item_selected(&mut self, key: u64, selected: bool) {
        if selected {
            self.selected_items.insert(key);
        } else {
            self.selected_items.remove(&key);
        }
    }

    // Toggle the selection state of the item at the cursor
    pub fn toggle_cursor_selected(&mut self) {
        if let Some(key) = self.get_cursor_item().map(Keyed::get_key) {
            self.set_item_selected(key, !self.get_item_selected(key));
        }
    }

    // Set the selected state of all items
    pub fn set_all_selected(&mut self, new_selected: bool) {
        if new_selected {
            self.selected_items = self.items.iter().map(Keyed::get_key).collect();
        } else {
            self.selected_items.clear();
        }
    }

    // Return a vector of the selected items
    pub fn get_selected_items(&self) -> impl Iterator<Item = &Item> {
        self.items
            .iter()
            .filter(|item| self.get_item_selected(item.get_key()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Item = u64;

    // Create a MultiselectList containing a certain number of unselected items
    fn get_sized_list(size: usize) -> MultiselectList<Item> {
        let mut list = MultiselectList::<Item>::new();
        list.replace_items((0..size).map(|index| index as Item).collect());
        list
    }

    // Start with a list with no items selected, then move the cursor and
    // return a vector if the indexes of the selected items
    fn set_cursor_helper(
        mut list: MultiselectList<Item>,
        start: Option<usize>,
        end: Option<usize>,
        selection_mode: SelectionMode,
    ) -> Vec<bool> {
        list.set_cursor(start);
        list.selection_mode = selection_mode;
        list.set_cursor(end);
        list.items
            .into_iter()
            .map(|item| list.selected_items.contains(&item.get_key()))
            .collect()
    }

    #[test]
    fn test_set_cursor_select() {
        assert_eq!(
            set_cursor_helper(get_sized_list(5), Some(0), Some(1), SelectionMode::Select),
            vec![true, true, false, false, false]
        );

        assert_eq!(
            set_cursor_helper(get_sized_list(5), Some(1), Some(3), SelectionMode::Select),
            vec![false, true, true, true, false]
        );

        assert_eq!(
            set_cursor_helper(get_sized_list(5), Some(2), Some(0), SelectionMode::Select),
            vec![true, true, true, false, false]
        );

        assert_eq!(
            set_cursor_helper(get_sized_list(5), Some(2), Some(2), SelectionMode::Select),
            vec![false, false, true, false, false]
        );

        assert_eq!(
            set_cursor_helper(get_sized_list(5), None, Some(2), SelectionMode::Select),
            vec![true, true, true, false, false]
        );

        assert_eq!(
            set_cursor_helper(get_sized_list(5), Some(2), None, SelectionMode::Select),
            vec![false, false, false, false, false]
        );
    }

    #[test]
    fn test_set_cursor_deselect() {
        let mut list = get_sized_list(5);
        list.set_all_selected(true);
        assert_eq!(
            set_cursor_helper(list, Some(1), Some(3), SelectionMode::Deselect),
            vec![true, false, false, false, true]
        );
    }

    #[test]
    fn test_set_cursor_no_select() {
        let list = get_sized_list(5);
        assert_eq!(
            set_cursor_helper(list, Some(1), Some(3), SelectionMode::None),
            vec![false, false, false, false, false]
        );

        let mut list = get_sized_list(5);
        list.set_all_selected(true);
        assert_eq!(
            set_cursor_helper(list, Some(1), Some(3), SelectionMode::None),
            vec![true, true, true, true, true]
        );
    }

    #[test]
    fn test_replace_items_selected() {
        let mut list = get_sized_list(5);
        list.set_item_selected(list.items.get(1).unwrap().get_key(), true);
        list.set_item_selected(list.items.get(2).unwrap().get_key(), true);
        list.replace_items(vec![0, 1, 3]);
        assert_eq!(
            list.get_items()
                .iter()
                .map(|item| list.get_item_selected(item.get_key()))
                .collect::<Vec<_>>(),
            vec![false, true, false]
        );
    }

    #[test]
    fn test_replace_items_selection() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(1));
        list.selection_mode = SelectionMode::Select;
        list.set_cursor(Some(3));
        list.replace_items(vec![0, 4]);
        assert_eq!(list.get_selected_items().count(), 0);
    }

    #[test]
    fn test_toggle_cursor_selected() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));

        list.toggle_cursor_selected();
        assert!(list.get_item_selected(list.items.get(2).unwrap().get_key()));

        list.toggle_cursor_selected();
        assert!(!list.get_item_selected(list.items.get(2).unwrap().get_key()));
    }

    #[test]
    fn test_set_all_selected() {
        let mut list = get_sized_list(2);

        list.set_all_selected(true);
        assert_eq!(list.selected_items.len(), 2);

        list.set_all_selected(false);
        assert_eq!(list.selected_items.len(), 0);
    }

    #[test]
    fn get_selected_items() {
        let mut list = get_sized_list(5);

        assert_eq!(
            list.get_selected_items().copied().collect::<Vec<_>>(),
            Vec::<Item>::new()
        );

        list.set_cursor(Some(2));
        list.toggle_cursor_selected();
        assert_eq!(
            list.selected_items.iter().copied().collect::<Vec<_>>(),
            vec![2]
        );

        list.set_all_selected(true);
        assert_eq!(
            list.get_selected_items().copied().collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
    }
}
