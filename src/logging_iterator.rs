/// Chunk iterator over a string slice scaled to newline characters.
///
/// This iterator will yield string slices below the maximum byte length
/// specified. If there are newline characters in a chunk, it will split after
/// the last of them instead of at the maximum length.
///
/// This is following the C implementation of the pmsg writer in Android:
/// https://cs.android.com/android/platform/superproject/+/master:system/logging/liblog/pmsg_writer.cpp;l=165
pub(crate) struct NewlineScaledChunkIterator<'a> {
    data: &'a str,
    max_byte_length: usize,
}

impl<'a> NewlineScaledChunkIterator<'a> {
    /// Create a new iterator instance.
    #[allow(dead_code)]
    pub fn new(data: &'a str, max_byte_length: usize) -> Self {
        Self { data, max_byte_length }
    }
}

impl<'a> Iterator for NewlineScaledChunkIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        // We yield all or split depending on the byte-length,
        // *not* the character length.
        match self.data.as_bytes().len() {
            0 => None,
            x if x < self.max_byte_length => {
                let last_piece = self.data;
                self.data = "";
                Some(last_piece)
            }
            _length_above_limit => {
                // Find char boundary before the max length
                let split_idx = find_char_boundary_before_idx(self.data, self.max_byte_length);

                // Try to find a newline char before the split point
                let split_idx = match self.data[..split_idx].rfind('\n') {
                    Some(byte_idx) => byte_idx + 1, // *After* the newline
                    None => split_idx,
                };

                // Split slice into a piece to return and a remainder that is updated in the iterator
                let (next_piece, remainder) = self.data.split_at(split_idx);
                self.data = remainder;

                Some(next_piece)
            }
        }
    }
}

/// Find the character boundary before an index in a string slice.
fn find_char_boundary_before_idx(data: &str, mut idx: usize) -> usize {
    loop {
        match data.is_char_boundary(idx) {
            true => return idx,
            false => {
                idx -= 1;
                if idx == 0 {
                    return idx;
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_find_char_boundary() {
        let test_str = "World 和 is what we want";
        let idx = 7; // inside the Chinese 'peace' sign

        let char_boundary_before = find_char_boundary_before_idx(test_str, idx);
        assert_eq!(char_boundary_before, 6);

        let (first_part, second_part) = test_str.split_at(char_boundary_before);
        assert_eq!(first_part, "World ");
        assert_eq!(second_part, "和 is what we want");
    }

    #[test]
    fn test_newline_scaled_chunk_iterator() {
        let test_str = "This will be a long string.\n\
                              Break it at the last newline below 50 bytes.\n\
                              This may split words into two.\n\
                              Some chunks are also above the maximum length \
                              without a newline and will be split at the \
                              charater boundary below the maximum length.";

        let mut nl_iter = NewlineScaledChunkIterator::new(test_str, 50);
        assert_eq!(nl_iter.next(), Some("This will be a long string.\n"));
        assert_eq!(nl_iter.next(), Some("Break it at the last newline below 50 bytes.\n"));
        assert_eq!(nl_iter.next(), Some("This may split words into two.\n"));
        assert_eq!(nl_iter.next(), Some("Some chunks are also above the maximum length with"));
        assert_eq!(nl_iter.next(), Some("out a newline and will be split at the charater bo"));
        assert_eq!(nl_iter.next(), Some("undary below the maximum length."));
        assert_eq!(nl_iter.next(), None);
    }
}
