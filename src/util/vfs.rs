use bevy::platform::collections::{Equivalent, HashMap};
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::hash::Hash;

pub struct Vfs<T> {
  inner: HashMap<VfsPath, VfsDir<T>>,
}

impl<T> Default for Vfs<T> {
  fn default() -> Self {
    Self { inner: default() }
  }
}

impl<T> Vfs<T> {
  pub fn new() -> Self {
    Self { inner: default() }
  }

  pub fn create(&mut self, path: impl Into<VfsPath>) -> &mut VfsDir<T> {
    let path = path.into();
    if let Some((parent_ref, basename)) = path.parent_ref().zip(path.basename()) {
      self.create(parent_ref).add_dir(basename);
    }
    self.inner.entry(path).or_default()
  }

  pub fn get_dir<P>(&self, path: P) -> Option<&VfsDir<T>>
  where
    P: Eq + Hash + Equivalent<VfsPath>,
  {
    self.inner.get(&path)
  }

  pub fn get_node<P>(&self, path: P) -> Option<&VfsNode<T>>
  where
    P: AsRef<VfsPath>,
  {
    let path_ref = path.as_ref();
    path_ref
      .parent_ref()
      .zip(path_ref.basename())
      .and_then(|(parent, item)| self.inner.get(&parent).and_then(|dir| dir.get(item)))
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VfsPath, &VfsDir<T>)> {
    self.inner.iter()
  }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct VfsPath<T = String>(Vec<T>)
where
  T: Eq + Hash;

impl<T> VfsPath<T>
where
  T: Eq + Hash,
{
  pub fn push(&mut self, item: T) {
    self.0.push(item);
  }

  pub fn iter(&self) -> impl Iterator<Item = &T> {
    self.0.iter()
  }
}

impl<T> VfsPath<T>
where
  T: Eq + Hash + Clone,
{
  pub fn has_parent(&self) -> bool {
    !self.0.is_empty()
  }

  pub fn parent(&self) -> Option<Self> {
    match self.0.len() {
      0 => None,
      1 => Some(Self(Vec::new())),
      n => {
        let slice = &self.0[0..n - 1];
        Some(Self(Vec::from(slice)))
      }
    }
  }
}

impl VfsPath {
  pub fn parent_ref(&self) -> Option<VfsPath<&str>> {
    match self.0.len() {
      0 => None,
      1 => Some(VfsPath(Vec::new())),
      n => {
        let refs = self.0[0..n - 1]
          .iter()
          .map(String::as_str)
          .collect::<Vec<_>>();
        Some(VfsPath(refs))
      }
    }
  }
}

impl<T> Default for VfsPath<T>
where
  T: Eq + Hash,
{
  fn default() -> Self {
    Self(Vec::new())
  }
}

impl AsRef<VfsPath> for VfsPath {
  fn as_ref(&self) -> &VfsPath {
    self
  }
}

impl<S> VfsPath<S>
where
  S: Eq + Hash + AsRef<str>,
{
  pub fn basename(&self) -> Option<&str> {
    if !self.0.is_empty() {
      self.0.last().map(S::as_ref)
    } else {
      None
    }
  }
}

impl From<VfsPath<&str>> for VfsPath {
  fn from(value: VfsPath<&str>) -> Self {
    Self(value.0.into_iter().map(String::from).collect())
  }
}

impl<const N: usize> From<[&str; N]> for VfsPath {
  fn from(value: [&str; N]) -> Self {
    Self(Vec::from(value.map(str::to_string)))
  }
}

impl<'s, const N: usize> From<[&'s str; N]> for VfsPath<&'s str> {
  fn from(value: [&'s str; N]) -> Self {
    Self(Vec::from(value))
  }
}

impl<'s> From<Vec<&'s str>> for VfsPath<&'s str> {
  fn from(value: Vec<&'s str>) -> Self {
    Self(value)
  }
}

impl<const N: usize> Equivalent<VfsPath> for [&str; N] {
  fn equivalent(&self, key: &VfsPath) -> bool {
    key.0.as_slice() == self
  }
}

impl Equivalent<VfsPath> for VfsPath<&str> {
  fn equivalent(&self, key: &VfsPath) -> bool {
    self.0 == key.0
  }
}

impl Equivalent<VfsPath> for &VfsPath<&str> {
  fn equivalent(&self, key: &VfsPath) -> bool {
    self.0 == key.0
  }
}

impl Equivalent<VfsPath> for &VfsPath {
  fn equivalent(&self, key: &VfsPath) -> bool {
    self.0 == key.0
  }
}

pub enum VfsNode<T> {
  Dir(String),
  Item { name: String, value: T },
}

impl<T> VfsNode<T> {
  pub fn name(&self) -> &str {
    match self {
      VfsNode::Dir(name) => name,
      VfsNode::Item { name, .. } => name,
    }
  }
}

impl<T> PartialEq for VfsNode<T> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (VfsNode::Dir(name), VfsNode::Dir(other_name)) => name.eq(other_name),
      (
        VfsNode::Dir(dir_name),
        VfsNode::Item {
          name: item_name, ..
        },
      ) => dir_name.eq(item_name),
      (
        VfsNode::Item {
          name: item_name, ..
        },
        VfsNode::Dir(dir_name),
      ) => item_name.eq(dir_name),
      (
        VfsNode::Item { name, .. },
        VfsNode::Item {
          name: other_name, ..
        },
      ) => name.eq(other_name),
    }
  }
}

impl<T> Eq for VfsNode<T> {}

impl<T> PartialOrd for VfsNode<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<T> Ord for VfsNode<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (VfsNode::Dir(name), VfsNode::Dir(other_name)) => name.cmp(other_name),
      (VfsNode::Dir(..), VfsNode::Item { .. }) => Ordering::Less,
      (VfsNode::Item { .. }, VfsNode::Dir(..)) => Ordering::Greater,
      (
        VfsNode::Item { name, .. },
        VfsNode::Item {
          name: other_name, ..
        },
      ) => name.cmp(other_name),
    }
  }
}

impl<T> Clone for VfsNode<T>
where
  T: Clone,
{
  fn clone(&self) -> Self {
    match self {
      Self::Dir(dir) => Self::Dir(dir.clone()),
      Self::Item { name, value } => Self::Item {
        name: name.clone(),
        value: value.clone(),
      },
    }
  }
}

impl<T> Debug for VfsNode<T>
where
  T: Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VfsNode::Dir(dir) => f
        .debug_tuple(std::any::type_name::<Self>())
        .field(&dir)
        .finish(),
      VfsNode::Item { name, value } => f
        .debug_struct(std::any::type_name::<Self>())
        .field("name", name)
        .field("value", value)
        .finish(),
    }
  }
}

pub struct VfsDir<T> {
  nodes: BTreeSet<VfsNode<T>>,
}

impl<T> Default for VfsDir<T> {
  fn default() -> Self {
    Self { nodes: default() }
  }
}

impl<T> VfsDir<T> {
  pub fn add(&mut self, node: impl Into<VfsNode<T>>) {
    self.nodes.insert(node.into());
  }

  pub fn add_dir(&mut self, name: impl Into<String>) {
    self.add(VfsNode::Dir(name.into()));
  }

  pub fn add_item(&mut self, name: impl Into<String>, item: T) {
    self.add(VfsNode::Item {
      name: name.into(),
      value: item,
    });
  }

  pub fn get(&self, item_name: &str) -> Option<&VfsNode<T>> {
    self.nodes.iter().find(|n| n.name() == item_name)
  }

  pub fn iter(&self) -> impl Iterator<Item = &VfsNode<T>> {
    self.nodes.iter()
  }
}

impl<T> Clone for VfsDir<T>
where
  T: Clone,
{
  fn clone(&self) -> Self {
    Self {
      nodes: self.nodes.clone(),
    }
  }
}

impl<T> Debug for VfsDir<T>
where
  T: Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct(std::any::type_name::<Self>())
      .field("nodes", &self.nodes)
      .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::{Vfs, VfsNode, VfsPath};

  #[test]
  fn vfs_add_dir() {
    let mut vfs = Vfs::new();

    vfs.create(["dir"]).add_item("item", 1);
    vfs.create(["dir2"]);

    let VfsNode::Item { value, .. } = vfs.get_node(VfsPath::from(["dir", "item"])).unwrap() else {
      panic!("Item not an item");
    };

    assert_eq!(*value, 1);

    let root = vfs.get_dir([]).unwrap();

    let VfsNode::Dir(dir) = root.get("dir").unwrap() else {
      panic!("Dir is not a dir")
    };

    assert_eq!(dir, "dir");

    let VfsNode::Dir(dir) = root.get("dir2").unwrap() else {
      panic!("Dir is not a dir")
    };

    assert_eq!(dir, "dir2");
  }
}
