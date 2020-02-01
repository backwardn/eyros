extern crate eyros;
extern crate failure;
extern crate random;
extern crate random_access_disk;
extern crate tempfile;

use eyros::{DB,Row,Location};
use failure::Error;
use random_access_disk::RandomAccessDisk;
use random::{Source,default as rand};
use tempfile::Builder as Tmpfile;

use std::cmp::Ordering;
use std::time;
use std::collections::HashSet;

type P = ((f32,f32),(f32,f32),f32);
type V = u32;

#[test]
fn delete() -> Result<(),Error> {
  let dir = Tmpfile::new().prefix("eyros").tempdir()?;
  let mut db: DB<_,_,P,V> = DB::open(
    |name: &str| -> Result<RandomAccessDisk,Error> {
      let p = dir.path().join(name);
      Ok(RandomAccessDisk::builder(p)
        .auto_sync(false)
        .build()?)
    }
  )?;
  let mut r = rand().seed([13,12]);
  let size = 40_000;
  let inserts: Vec<Row<P,V>> = (0..size).map(|_| {
    let xmin: f32 = r.read::<f32>()*2.0-1.0;
    let xmax: f32 = xmin + r.read::<f32>().powf(64.0)*(1.0-xmin);
    let ymin: f32 = r.read::<f32>()*2.0-1.0;
    let ymax: f32 = ymin + r.read::<f32>().powf(64.0)*(1.0-ymin);
    let time: f32 = r.read::<f32>()*1000.0;
    let value: u32 = r.read();
    let point = ((xmin,xmax),(ymin,ymax),time);
    Row::Insert(point, value)
  }).collect();
  let batch_size = 5_000;
  let n = size / batch_size;
  let batches: Vec<Vec<Row<P,V>>> = (0..n).map(|i| {
    inserts[i*batch_size..(i+1)*batch_size].to_vec()
  }).collect();
  eprintln!["inserts.len()={}", inserts.len()];
  {
    let mut total = 0f64;
    for batch in batches {
      let start = time::Instant::now();
      db.batch(&batch)?;
      let elapsed = start.elapsed().as_secs_f64();
      total += elapsed;
      eprintln!["batch write for {} records in {} seconds",
        batch.len(), elapsed];
    }
    eprintln!["total batch time: {}\nwrote {} records per second",
      total, (size as f64)/total];
  }

  let full: Vec<(P,V,Location)> = {
    let bbox = ((-1.0,-1.0,0.0),(1.0,1.0,1000.0));
    let mut results = vec![];
    for result in db.query(&bbox)? {
      results.push(result?);
    }
    results
  };
  assert_eq![full.len(), inserts.len(),
    "correct number of results before deleting"];

  let mut deleted: HashSet<Location> = HashSet::new();
  {
    // delete 1/10th of the records
    let mut deletes = vec![];
    for (i,r) in full.iter().enumerate() {
      if i % 10 == 0 {
        deletes.push(Row::Delete(r.2));
        deleted.insert(r.2);
      }
    };
    let start = time::Instant::now();
    db.batch(&deletes)?;
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!["batch delete for {} records in {} seconds",
      deletes.len(), elapsed];
  }

  {
    let bbox = ((-1.0,-1.0,0.0),(1.0,1.0,1000.0));
    let mut results = vec![];
    let start = time::Instant::now();
    for result in db.query(&bbox)? {
      results.push(result?);
    }
    eprintln!["query for {} records in {} seconds",
      results.len(), start.elapsed().as_secs_f64()];
    assert_eq!(results.len(), size, "incorrect length for full region");
    let mut expected: Vec<(P,V,Location)> = full.iter()
      .filter(|r| !deleted.contains(&r.2))
      .map(|r| *r)
      .collect();
    results.sort_unstable_by(cmp);
    expected.sort_unstable_by(cmp);
    assert_eq!(results.len(), expected.len(),
      "incorrect length for full result");
    assert_eq!(results, expected, "incorrect results for full region");
  }

  {
    let bbox = ((-0.8,0.1,0.0),(0.2,0.5,500.0));
    let mut results = vec![];
    let start = time::Instant::now();
    for result in db.query(&bbox)? {
      results.push(result?)
    }
    eprintln!["query for {} records in {} seconds",
      results.len(), start.elapsed().as_secs_f64()];
    let mut expected: Vec<(P,V,Location)> = full.iter()
      .filter(|r| {
        !deleted.contains(&r.2)
        && contains_iv((bbox.0).0,(bbox.1).0, (r.0).0)
        && contains_iv((bbox.0).1,(bbox.1).1, (r.0).1)
        && contains_pt((bbox.0).2,(bbox.1).2, (r.0).2)
      })
      .map(|r| *r)
      .collect();
    results.sort_unstable_by(cmp);
    expected.sort_unstable_by(cmp);
    assert_eq!(results.len(), expected.len(),
      "incorrect length for partial region");
    assert_eq!(results, expected, "incorrect results for partial region");
  }

  {
    let bbox = ((-0.500,0.800,200.0),(-0.495,0.805,300.0));
    let mut results = vec![];
    let start = time::Instant::now();
    for result in db.query(&bbox)? {
      results.push(result?);
    }
    eprintln!["query for {} records in {} seconds",
      results.len(), start.elapsed().as_secs_f64()];
    let mut expected: Vec<(P,V,Location)> = full.iter()
      .filter(|r| {
        !deleted.contains(&r.2)
        && contains_iv((bbox.0).0,(bbox.1).0, (r.0).0)
        && contains_iv((bbox.0).1,(bbox.1).1, (r.0).1)
        && contains_pt((bbox.0).2,(bbox.1).2, (r.0).2)
      })
      .map(|r| *r)
      .collect();
    results.sort_unstable_by(cmp);
    expected.sort_unstable_by(cmp);
    assert_eq!(results.len(), expected.len(),
      "incorrect length for small region");
    assert_eq!(results, expected, "incorrect results for small region");
  }
  Ok(())
}

fn cmp<T> (a: &T, b: &T) -> Ordering where T: PartialOrd {
  match a.partial_cmp(b) {
    Some(o) => o,
    None => panic!["comparison failed"]
  }
}

fn contains_iv<T> (min: T, max: T, iv: (T,T)) -> bool where T: PartialOrd {
  min <= iv.1 && iv.0 <= max
}
fn contains_pt<T> (min: T, max: T, pt: T) -> bool where T: PartialOrd {
  min <= pt && pt <= max
}
