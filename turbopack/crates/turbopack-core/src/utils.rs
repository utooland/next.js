use std::{
    cmp::min, collections::HashSet, future::Future, hash::Hash, pin::Pin, sync::LazyLock,
    task::Poll,
};

use regex::Regex;

pub fn _race_pop<'a, T: 'a, F: Future<Output = T> + Unpin>(
    futures: &'a mut Vec<F>,
) -> impl Future<Output = Option<T>> + 'a {
    _FutureRacePop { futures }
}

struct _FutureRacePop<'a, T, F: Future<Output = T> + Unpin> {
    futures: &'a mut Vec<F>,
}

impl<T, F: Future<Output = T> + Unpin> Future for _FutureRacePop<'_, T, F> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.futures.is_empty() {
            return Poll::Ready(None);
        }
        match self.futures.iter_mut().enumerate().find_map(|(i, future)| {
            match Pin::new(future).poll(cx) {
                Poll::Ready(res) => Some((i, res)),
                Poll::Pending => None,
            }
        }) {
            Some((i, res)) => {
                self.futures.swap_remove(i);
                Poll::Ready(Some(res))
            }
            None => Poll::Pending,
        }
    }
}
pub async fn _visit<N, V, F, R, L, G, T>(node: N, visit: V, get_referenced_nodes: R) -> Vec<T>
where
    N: Clone + Hash + PartialEq + Eq,
    V: Fn(N) -> F,
    F: Future<Output = T>,
    R: Fn(N) -> G,
    L: IntoIterator<Item = N>,
    G: Future<Output = L>,
{
    let mut visited = HashSet::new();
    let mut results = Vec::new();
    visited.insert(node.clone());
    let mut queue = vec![node];
    let mut futures_queue = Vec::new();
    loop {
        match queue.pop() {
            Some(node) => {
                results.push(visit(node.clone()).await);
                futures_queue.push(Box::pin(get_referenced_nodes(node)));
            }
            None => match _race_pop(&mut futures_queue).await {
                Some(iter) => {
                    for node in iter {
                        if !visited.contains(&node) {
                            visited.insert(node.clone());
                            queue.push(node.clone());
                        }
                    }
                }
                None => break,
            },
        }
    }
    assert!(futures_queue.is_empty());
    results
}

// ── Filename template placeholder utilities ──────────────────────────────────

static NAME_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[name\]").unwrap());

/// Returns true if the string contains a `[name]` placeholder.
pub fn match_name_placeholder(s: &str) -> bool {
    NAME_PLACEHOLDER_REGEX.is_match(s)
}

/// Replaces `[name]` placeholders in a filename template string.
///
/// If the name already ends with an extension (e.g. "foo.js") and the template
/// text right after `[name]` starts with that same extension (e.g. ".js"), the
/// extension is stripped from the name to avoid duplication like "foo.js.js".
pub fn replace_name_placeholder(s: &str, name: &str) -> String {
    NAME_PLACEHOLDER_REGEX
        .replace_all(s, |caps: &regex::Captures| {
            let m = caps.get(0).unwrap();
            let after = &s[m.end()..];
            if let Some(dot_pos) = name.rfind('.') {
                let ext = &name[dot_pos..];
                if after.starts_with(ext) {
                    return name[..dot_pos].to_string();
                }
            }
            name.to_string()
        })
        .to_string()
}

static CONTENT_HASH_PLACEHOLDER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[contenthash(?::(?P<len>\d+))?\]").unwrap());

/// Returns true if the string contains a `[contenthash]` or
/// `[contenthash:N]` placeholder.
pub fn match_content_hash_placeholder(s: &str) -> bool {
    CONTENT_HASH_PLACEHOLDER_REGEX.is_match(s)
}

/// Replaces `[contenthash]` / `[contenthash:N]` placeholders with the given
/// hash string. When a length `N` is specified, the hash is truncated to that
/// many characters.
pub fn replace_content_hash_placeholder(s: &str, hash: &str) -> String {
    CONTENT_HASH_PLACEHOLDER_REGEX
        .replace_all(s, |caps: &regex::Captures| {
            let len = caps.name("len").map(|m| m.as_str()).unwrap_or("");
            let len = if len.is_empty() {
                hash.len()
            } else {
                len.parse().unwrap_or(hash.len())
            };
            let len = min(len, hash.len());
            hash[..len].to_string()
        })
        .to_string()
}
