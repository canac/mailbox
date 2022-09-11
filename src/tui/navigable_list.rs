use std::collections::HashMap;

pub trait Keyed {
    // Return the unique key of an object
    fn get_key(&self) -> u64;
}

pub trait NavigableList<Item>
where
    Item: Keyed,
{
    fn get_items(&self) -> &Vec<Item>;
    fn get_items_mut(&mut self) -> &mut Vec<Item>;
    fn get_cursor(&self) -> Option<usize>;
    fn set_cursor(&mut self, cursor: Option<usize>);

    // Move the cursor to the first item
    fn first(&mut self) {
        self.set_cursor(if self.get_items().is_empty() {
            None
        } else {
            Some(0)
        });
    }

    // Move the cursor to the last item
    fn last(&mut self) {
        self.set_cursor(match self.get_items().len() {
            0 => None,
            count => Some(count - 1),
        });
    }

    // Move the cursor forward by `change` items (backward if `change` is negative)
    fn move_cursor_relative(&mut self, change: i64) {
        self.set_cursor(match self.get_items().len() {
            0 => None,
            num_items => {
                let new_index = (match self.get_cursor() {
                    None => -1,
                    Some(index) => index as i64,
                }) + change;
                // Bounds check the new index
                Some((new_index).clamp(0, num_items as i64 - 1) as usize)
            }
        });
    }

    // Move the cursor to the next item
    fn next(&mut self) {
        self.move_cursor_relative(1);
    }

    // Move the cursor to the previous item
    fn previous(&mut self) {
        self.move_cursor_relative(-1);
    }

    // Remove the cursor from any item
    fn remove_cursor(&mut self) {
        self.set_cursor(None);
    }

    // Return a reference to the item at the cursor
    fn get_cursor_item(&self) -> Option<&Item> {
        self.get_cursor()
            .and_then(|index| self.get_items().get(index))
    }

    // Return a mutable reference to the item at the cursor
    fn get_cursor_item_mut(&mut self) -> Option<&mut Item> {
        self.get_cursor()
            .and_then(|index| self.get_items_mut().get_mut(index))
    }

    // Replace the list's items with a new set of items
    fn replace_items(&mut self, items: Vec<Item>) {
        let cursor_candidates = match self.get_cursor() {
            None => vec![],
            Some(index) => self
                .get_items()
                .iter()
                .skip(index)
                .map(|item| item.get_key())
                .collect(),
        };

        *self.get_items_mut() = items;

        // Build a map of the new items' keys to indexes
        let new_keys = self
            .get_items()
            .iter()
            .enumerate()
            .map(|(index, item)| (item.get_key(), index))
            .collect::<HashMap<_, _>>();
        // Move the cursor to the first matching key
        self.set_cursor(
            cursor_candidates
                .into_iter()
                .find_map(|key| new_keys.get(&key))
                .cloned(),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::tui::navigable_list::NavigableList;

    use super::*;

    impl Keyed for u64 {
        fn get_key(&self) -> u64 {
            *self
        }
    }

    struct List {
        items: Vec<u64>,
        cursor: Option<usize>,
    }

    impl NavigableList<u64> for List {
        fn get_items(&self) -> &Vec<u64> {
            &self.items
        }

        fn get_items_mut(&mut self) -> &mut Vec<u64> {
            &mut self.items
        }

        fn get_cursor(&self) -> Option<usize> {
            self.cursor
        }

        fn set_cursor(&mut self, new_cursor: Option<usize>) {
            self.cursor = new_cursor;
        }
    }

    // Create a List containing a certain number of items
    fn get_sized_list(size: usize) -> List {
        List {
            items: (0..size).map(|index| index as u64).collect(),
            cursor: None,
        }
    }

    #[test]
    fn test_move_cursor() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        assert_eq!(list.cursor, Some(2));

        list.set_cursor(None);
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_replace_items_shift() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.replace_items(vec![0, 1, 3]);
        assert_eq!(list.cursor, Some(2));
    }

    #[test]
    fn test_replace_items_rearrange() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.replace_items(vec![3, 2, 1, 0]);
        assert_eq!(list.cursor, Some(1));
    }

    #[test]
    fn test_replace_items_remove() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.replace_items(vec![0, 1]);
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_first() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.first();
        assert_eq!(list.cursor, Some(0));
    }

    #[test]
    fn test_first_empty() {
        let mut list = get_sized_list(0);
        list.first();
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_last() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.last();
        assert_eq!(list.cursor, Some(4));
    }

    #[test]
    fn test_last_empty() {
        let mut list = get_sized_list(0);
        list.last();
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_move_cursor_relative() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));

        list.move_cursor_relative(2);
        assert_eq!(list.cursor, Some(4));

        list.move_cursor_relative(-3);
        assert_eq!(list.cursor, Some(1));
    }

    #[test]
    fn test_move_cursor_relative_past_bounds() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.move_cursor_relative(5);
        assert_eq!(list.cursor, Some(4));

        list.move_cursor_relative(-10);
        assert_eq!(list.cursor, Some(0));
    }

    #[test]
    fn test_move_cursor_relative_no_cursor() {
        let mut list = get_sized_list(5);

        list.move_cursor_relative(3);
        assert_eq!(list.cursor, Some(2));

        list.set_cursor(None);
        list.move_cursor_relative(-3);
        assert_eq!(list.cursor, Some(0));
    }

    #[test]
    fn test_move_cursor_relative_empty() {
        let mut list = get_sized_list(0);
        list.move_cursor_relative(1);
        assert_eq!(list.cursor, None);
        list.move_cursor_relative(-1);
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_remove_cursor() {
        let mut list = get_sized_list(5);
        list.set_cursor(Some(2));
        list.remove_cursor();
        assert_eq!(list.cursor, None);
    }

    #[test]
    fn test_get_cursor_item() {
        let mut list = get_sized_list(5);

        list.set_cursor(Some(2));
        assert_eq!(list.get_cursor_item(), Some(&2));

        list.set_cursor(None);
        assert_eq!(list.get_cursor_item(), None);
    }
}
