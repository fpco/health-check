const BUFFER_SIZE: usize = 8192;

pub(crate) struct LineHelper {
    buffer: [u8; BUFFER_SIZE],
    len: usize,
}

impl LineHelper {
    pub(crate) fn new() -> Self {
        LineHelper {
            buffer: [0; BUFFER_SIZE],
            len: 0,
        }
    }
    pub(crate) fn append(&mut self, new_data: &[u8]) -> impl Iterator<Item = String> {
        let mut res = vec![];

        if new_data.len() + self.len > BUFFER_SIZE {
            res.push(String::from_utf8_lossy(&self.buffer[..self.len]).into_owned());
            self.len = 0;
        }

        self.buffer[self.len..self.len + new_data.len()].copy_from_slice(new_data);
        self.len += new_data.len();

        while let Some(idx) = find_newline(&self.buffer[..self.len]) {
            let end = if idx > 0 && self.buffer[idx - 1] == b'\r' {
                idx - 1
            } else {
                idx
            };
            let line = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
            res.push(line);
            self.buffer.rotate_left(idx + 1);
            self.len -= idx + 1;
        }

        res.into_iter()
    }

    pub(crate) fn finish(self) -> Option<String> {
        if self.len == 0 {
            None
        } else {
            Some(String::from_utf8_lossy(&self.buffer[..self.len]).into_owned())
        }
    }
}

fn find_newline(s: &[u8]) -> Option<usize> {
    s.iter()
        .enumerate()
        .find_map(|(idx, c)| if *c == b'\n' { Some(idx) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line() {
        let mut helper = LineHelper::new();
        assert_eq!(
            helper.append(b"helloworld").collect::<Vec<_>>(),
            Vec::<String>::new()
        );
        assert_eq!(helper.finish(), Some("helloworld".to_owned()))
    }

    #[test]
    fn basic() {
        let mut helper = LineHelper::new();
        assert_eq!(
            helper.append(b"hello\nworld\r\n").collect::<Vec<_>>(),
            vec!["hello".to_owned(), "world".to_owned()]
        );
        assert_eq!(helper.finish(), None)
    }

    #[test]
    fn chunked() {
        let mut helper = LineHelper::new();
        assert_eq!(
            helper.append(b"he").collect::<Vec<_>>(),
            Vec::<String>::new()
        );
        assert_eq!(
            helper.append(b"ll").collect::<Vec<_>>(),
            Vec::<String>::new()
        );
        assert_eq!(
            helper.append(b"o\r\n").collect::<Vec<_>>(),
            vec!["hello".to_owned()]
        );
        assert_eq!(
            helper.append(b"world").collect::<Vec<_>>(),
            Vec::<String>::new()
        );
        assert_eq!(helper.finish(), Some("world".to_owned()))
    }
}
