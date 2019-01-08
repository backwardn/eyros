use point::Point;
use ::{Value};
use std::cmp::Ordering;

#[derive(Clone)]
pub enum Node<'a,P,V> where P: Point, V: Value {
  Branch(Branch<'a,P,V>),
  Data(Data<'a,P,V>)
}

pub trait Bytes {
  fn bytes (&self) -> usize;
}

#[derive(Clone)]
pub struct Data<'a,P,V> where P: Point, V: Value {
  offset: u64,
  bucket: Vec<usize>,
  rows: &'a Vec<&'a (P,V)>
}

impl<'a,P,V> Data<'a,P,V> where P: Point, V: Value {
  pub fn new (bucket: Vec<usize>, rows: &'a Vec<&'a (P,V)>) -> Self {
    Self { offset: 0, bucket, rows }
  }
  pub fn alloc (&mut self, alloc: &mut FnMut (usize) -> u64) -> () {
    self.offset = alloc(self.bytes());
  }
  pub fn data (&self) -> Vec<u8> {
    vec![]
  }
}

impl<'a,P,V> Bytes for Data<'a,P,V> where P: Point, V: Value {
  fn bytes (&self) -> usize {
    50
  }
}

impl<'a,P,V> Bytes for Branch<'a,P,V> where P: Point, V: Value {
  fn bytes (&self) -> usize {
    50
  }
}

impl<'a,P,V> Bytes for Node<'a,P,V> where P: Point, V: Value {
  fn bytes (&self) -> usize {
    match self {
      Node::Branch(b) => b.bytes(),
      Node::Data(d) => d.bytes()
    }
  }
}

#[derive(Clone)]
pub struct Branch<'a,P,V> where P: Point, V: Value {
  pub offset: u64,
  level: usize,
  max_data_size: usize,
  order: &'a Vec<usize>,
  bucket: Vec<usize>,
  buckets: Vec<Vec<usize>>,
  rows: &'a Vec<&'a (P,V)>,
  pivots: Vec<P>,
  sorted: Vec<usize>,
  intersecting: Vec<Vec<usize>>,
  matched: Vec<bool>
}

impl<'a,P,V> Branch<'a,P,V> where P: Point, V: Value {
  pub fn new (level: usize, max_data_size: usize, order: &'a Vec<usize>,
  bucket: Vec<usize>, rows: &'a Vec<&(P,V)>) -> Self {
    let n = order.len();
    let mut sorted: Vec<usize> = (0..bucket.len()).collect();
    sorted.sort_unstable_by(|a,b| {
      rows[bucket[*a]].0.cmp_at(&rows[bucket[*b]].0, level as usize)
    });
    let pivots: Vec<P> = (0..n).map(|k| {
      let m = ((k+2)*sorted.len()/(n+1)).min(sorted.len()-2);
      let a = &rows[bucket[sorted[m+0]]];
      let b = &rows[bucket[sorted[m+1]]];
      a.0.midpoint_upper(&b.0)
    }).collect();
    let mut intersecting = vec![vec![];n];
    let mut matched = vec![false;bucket.len()];
    for i in order.iter() {
      let pivot = pivots[*i];
      for j in sorted.iter() {
        let row = rows[bucket[*j]];
        if matched[*j] { continue }
        if row.0.cmp_at(&pivot, level as usize) == Ordering::Equal {
          matched[*j] = true;
          intersecting[*i].push(*j);
        }
      }
    }
    let n = order.len();
    Self {
      offset: 0,
      max_data_size,
      level,
      order,
      bucket,
      buckets: Vec::with_capacity(n),
      rows,
      pivots,
      sorted,
      intersecting,
      matched
    }
  }
  pub fn alloc (&mut self, alloc: &mut FnMut (usize) -> u64) -> () {
    self.offset = alloc(self.bytes());
  }
  pub fn build (&mut self, alloc: &mut FnMut (usize) -> u64)
  -> (Vec<u8>,Vec<Node<'a,P,V>>) {
    let n = self.order.len();
    self.buckets = vec![vec![];n];
    let bf = (n+1)/2;
    let mut j = 0;
    let mut pivot = self.pivots[self.order[bf-1]];
    for i in self.sorted.iter() {
      if self.matched[*i] { continue }
      let row = self.rows[self.bucket[*i]];
      while j < bf-1
      && row.0.cmp_at(&pivot, self.level as usize) != Ordering::Less {
        j = (j+1).min(bf-1);
        if j < bf-1 {
          pivot = self.pivots[self.order[j+bf-1]];
        }
      }
      self.buckets[j].push(*i);
    }
    let mut nodes = Vec::with_capacity(self.buckets.len());
    for bucket in self.buckets.iter() {
      if bucket.len() < self.max_data_size {
        let mut d = Data::new(bucket.clone(), self.rows);
        d.alloc(alloc);
        nodes.push(Node::Data(d));
      } else {
        let mut b = Branch::new(
          self.level+1, self.max_data_size, self.order,
          bucket.clone(), self.rows
        );
        b.alloc(alloc);
        nodes.push(Node::Branch(b));
      }
    }
    let data: Vec<u8> = vec![];
    (data,nodes)
  }
}