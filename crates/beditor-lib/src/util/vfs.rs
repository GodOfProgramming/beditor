use bevy::prelude::*;
use camino::Utf8PathBuf;
use core::cmp::Ordering;
use itertools::{FoldWhile, Itertools};
use petgraph::Graph;
use petgraph::graph::NodeIndex;
use std::borrow::Borrow;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

pub struct Vfs<T> {
  inner: Graph<VfsNode<T>, Relationship>,
  root: VfsPath,
}

impl<T> Default for Vfs<T> {
  fn default() -> Self {
    let mut graph = Graph::new();
    let root_index = graph.add_node(VfsNode::root());
    Self {
      inner: graph,
      root: VfsPath {
        cached: Name::new("/"),
        name: Name::new("/"),
        inner: Utf8PathBuf::from("/"),
        index: root_index,
      },
    }
  }
}

impl<T> Vfs<T> {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn root(&self) -> &VfsPath {
    &self.root
  }

  pub fn ls(&self, path: impl Borrow<VfsPath>) -> impl Iterator<Item = &VfsPath> {
    self.inner.edges(path.borrow().index).filter_map(|e| {
      if let Relationship::Child(dir) = e.weight() {
        Some(dir)
      } else {
        None
      }
    })
  }

  pub fn new_item(
    &mut self,
    path: impl Borrow<VfsPath>,
    name: impl Borrow<Name>,
    item: T,
  ) -> Option<&VfsPath> {
    self.new_node(path, name, VfsNode::Item { value: item }, false)
  }

  pub fn mkdir(&mut self, path: impl Borrow<VfsPath>, name: impl Borrow<Name>) -> Option<&VfsPath> {
    self.new_node(path, name, VfsNode::Dir, false)
  }

  /// Not very efficient due to lifetimes
  pub fn mkdir_p<N>(&mut self, mut path: impl Iterator<Item = N>, force: bool) -> Option<VfsPath>
  where
    N: Into<Name>,
  {
    let root = self.root().clone();
    path
      .fold_while(Some(root), |prev, next| {
        if let Some(prev) = prev
          && let Some(dir) = self
            .new_node(prev, next.into(), VfsNode::Dir, force)
            .cloned()
        {
          FoldWhile::Continue(Some(dir))
        } else {
          FoldWhile::Done(None)
        }
      })
      .into_inner()
  }

  pub fn read(&self, path: &VfsPath) -> Option<&VfsNode<T>> {
    self.inner.node_weight(path.index)
  }

  pub fn write(&mut self, path: &VfsPath) -> Option<&mut VfsNode<T>> {
    self.inner.node_weight_mut(path.index)
  }

  pub fn rm(&mut self, path: &VfsPath) -> Option<VfsNode<T>> {
    self.inner.remove_node(path.index)
  }

  pub fn iter(&self, path: &VfsPath) -> impl Iterator<Item = &VfsPath> {
    self.ls(path)
  }

  fn add_child(&mut self, parent: &VfsPath, child_name: &Name, node: VfsNode<T>) -> &VfsPath {
    let child_path = parent.join(child_name);
    let child_index = self.inner.add_node(node);

    let path = VfsPath {
      name: child_name.clone(),
      cached: Name::new(child_path.to_string()),
      inner: child_path,
      index: child_index,
    };

    let child_weight = self
      .inner
      .add_edge(parent.index, child_index, Relationship::Child(path));

    self.inner.add_edge(
      child_index,
      parent.index,
      Relationship::Parent(parent.clone()),
    );

    &*self
      .inner
      .edge_weight(child_weight)
      .expect("Edge was just added")
  }

  fn new_node(
    &mut self,
    path: impl Borrow<VfsPath>,
    name: impl Borrow<Name>,
    node: VfsNode<T>,
    force: bool,
  ) -> Option<&VfsPath> {
    let path = path.borrow();
    let name = name.borrow();

    if let Some(child_path) = self.find_child_by_name(path, name) {
      if force {
        let path = child_path.clone();
        self.rm(&path);
      } else {
        return None;
      }
    }

    let child = self.add_child(path, name, node);

    Some(child)
  }

  fn find_child_by_name(&self, path: &VfsPath, name: &Name) -> Option<&VfsPath> {
    self.inner.edges(path.index).find_map(|e| {
      if e.weight().name == *name {
        Some(e.weight().deref())
      } else {
        None
      }
    })
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Relationship {
  /// This edge points to the node's parent
  Parent(VfsPath),
  /// This edge points to one of a node's children
  Child(VfsPath),
}

impl Deref for Relationship {
  type Target = VfsPath;

  fn deref(&self) -> &Self::Target {
    match self {
      Relationship::Parent(vfs_path) => vfs_path,
      Relationship::Child(vfs_path) => vfs_path,
    }
  }
}

#[derive(Clone, Debug, Eq, Hash)]
pub struct VfsPath {
  cached: Name,
  name: Name,
  inner: Utf8PathBuf,
  index: NodeIndex,
}

impl VfsPath {
  pub fn join(&self, name: impl Borrow<Name>) -> Utf8PathBuf {
    self.inner.join(name.borrow().as_str())
  }

  pub fn has_parent<T>(&self, vfs: &Vfs<T>) -> bool {
    vfs
      .inner
      .edges(self.index)
      .find(|e| matches!(e.weight(), Relationship::Parent(_)))
      .is_some()
  }

  pub fn parent<'v, T>(&self, vfs: &'v Vfs<T>) -> Option<&'v Self> {
    vfs.inner.edges(self.index).find_map(|e| {
      if let Relationship::Parent(path) = e.weight() {
        Some(path)
      } else {
        None
      }
    })
  }

  pub fn full_path(&self) -> &str {
    self.cached.as_str()
  }

  pub fn basename(&self) -> &str {
    self.name.as_str()
  }
}

impl PartialEq for VfsPath {
  fn eq(&self, other: &Self) -> bool {
    self.cached == other.cached && self.index == other.index
  }
}

impl PartialOrd for VfsPath {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.cached.partial_cmp(&other.cached)
  }
}

impl Ord for VfsPath {
  fn cmp(&self, other: &Self) -> Ordering {
    self.cached.cmp(&other.cached)
  }
}

pub enum VfsNode<T> {
  Dir,
  Item { value: T },
}

impl<T> VfsNode<T> {
  pub fn root() -> Self {
    Self::Dir
  }
}

impl<T> Clone for VfsNode<T>
where
  T: Clone,
{
  fn clone(&self) -> Self {
    match self {
      Self::Dir => Self::Dir,
      Self::Item { value } => Self::Item {
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
      VfsNode::Dir => f.debug_tuple(std::any::type_name::<Self>()).finish(),
      VfsNode::Item { value } => f
        .debug_struct(std::any::type_name::<Self>())
        .field("value", value)
        .finish(),
    }
  }
}
