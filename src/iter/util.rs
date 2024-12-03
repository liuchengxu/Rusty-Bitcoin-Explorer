/// A lightweight data structure for storing unspent transaction outputs (UTXOs).
///
/// The `VecMap` provides efficient in-memory storage of UTXOs where each entry is either
/// `Some(Box<T>)` (representing a UTXO) or `None` (representing an empty slot). The vector
/// is compact and allows fast access, removal, and checks for emptiness.
///
/// This is used when the feature `"on-disk-utxo"` is **not** enabled.
#[cfg(not(feature = "on-disk-utxo"))]
pub(crate) struct VecMap<T> {
    size: u32,
    inner: Box<[Option<Box<T>>]>,
}

#[cfg(not(feature = "on-disk-utxo"))]
impl<T> VecMap<T> {
    /// Creates a new `VecMap` from a vector of optional boxed elements.
    ///
    /// # Arguments
    /// * `slice`: A boxed slice containing `Option<Box<T>>` elements.
    ///
    /// # Returns
    /// A new `VecMap` containing the provided data.
    #[inline(always)]
    pub(crate) fn from_vec(slice: Box<[Option<Box<T>>]>) -> Self {
        Self {
            size: slice.len() as u32,
            inner: slice,
        }
    }

    /// Returns `true` if the `VecMap` is empty.
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Removes and returns an element from the `VecMap` at the given index.
    ///
    /// If the element exists, it will be removed, the size of the map will be decremented,
    /// and `Some(Box<T>)` will be returned. If no element exists at the given index (out of bounds or `None`),
    /// it will return `None`.
    ///
    /// # Arguments
    /// * `index`: The index at which to remove an element.
    ///
    /// # Returns
    /// `Some(Box<T>)` if an element was removed, `None` otherwise.
    pub(crate) fn remove(&mut self, index: usize) -> Option<Box<T>> {
        // Ensure the index is within bounds
        if index >= self.inner.len() {
            return None; // Returning None if index is out of bounds
        }

        // Take the element from the inner vector
        let element = self.inner[index].take();

        // Decrement the size if an element was removed
        if element.is_some() {
            self.size -= 1;
        }

        element
    }
}

#[cfg(test)]
#[cfg(not(feature = "on-disk-utxo"))]
mod test_vec_map {
    use crate::api::CompactTxOut;
    use crate::iter::util::VecMap;
    use bitcoin::TxOut;

    #[test]
    fn test_vec_map() {
        // Initialize a VecMap with some test data
        let mut vec: VecMap<CompactTxOut> = VecMap::from_vec(
            vec![
                Some(Box::new(TxOut::NULL.into())),
                Some(Box::new(TxOut::NULL.into())),
                Some(Box::new(TxOut::NULL.into())),
            ]
            .into_boxed_slice(),
        );

        // Check the initial size.
        assert_eq!(vec.size, 3);

        // Remove an element from index 1 and check the size.
        assert!(vec.remove(1).is_some());
        assert_eq!(vec.size, 2);

        // Try to remove an element at index 1 again, which should be empty.
        assert!(vec.remove(1).is_none());
        assert_eq!(vec.size, 2);

        // Remove an element from index 0 and check the size.
        assert!(vec.remove(0).is_some());
        assert_eq!(vec.size, 1);

        // Try removing from index 0 again, which should be empty.
        assert!(vec.remove(0).is_none());
        assert_eq!(vec.size, 1);

        // Ensure the vector is not empty.
        assert!(!vec.is_empty());

        // Try to remove an element from index 2 (which is out of bounds after the removals)
        assert!(vec.remove(2).is_some());

        // Ensure the vector is now empty
        assert!(vec.is_empty());
        assert_eq!(vec.size, 0);
    }

    #[test]
    fn test_remove_out_of_bounds() {
        let mut vec: VecMap<CompactTxOut> = VecMap::from_vec(
            vec![
                Some(Box::new(TxOut::NULL.into())),
                Some(Box::new(TxOut::NULL.into())),
            ]
            .into_boxed_slice(),
        );

        // Ensure we get None when trying to remove from an invalid index
        assert!(vec.remove(3).is_none()); // Index 3 is out of bounds
    }
}
