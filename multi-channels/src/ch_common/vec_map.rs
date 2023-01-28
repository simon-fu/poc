
// 第一种 VecSet 实现方式
pub type VecSet<T> = indexmap::IndexSet<T>;
pub type VecMap<K, T> = indexmap::IndexMap<K, T>;


// 第二种 VecSet 实现方式
// refer from https://stackoverflow.com/questions/53755017/can-i-randomly-sample-from-a-hashset-efficiently



// use std::collections::HashSet;

// #[derive(Default)]
// pub struct VecSet<T> {
//     // refer from https://stackoverflow.com/questions/53755017/can-i-randomly-sample-from-a-hashset-efficiently
//     set: HashSet<T>,
//     vec: Vec<T>,
// }

// impl<T> VecSet<T>
// where
//     T: Clone + Eq + std::hash::Hash,
// {
//     pub fn new() -> Self {
//         Self {
//             set: HashSet::new(),
//             vec: Vec::new(),
//         }
//     }

//     pub fn is_empty(&self) -> bool {
//         assert_eq!(self.set.len(), self.vec.len());
//         self.vec.is_empty()
//     }

//     pub fn len(&self) -> usize {
//         self.vec.len()
//     }

//     pub fn insert(&mut self, v: T) -> bool {
//         assert_eq!(self.set.len(), self.vec.len());
//         let was_new = self.set.insert(v.clone());
//         if was_new {
//             self.vec.push(v);
//         }
//         was_new
//     }

//     pub fn remove(&mut self, v: &T) -> bool {
//         let removed = self.set.remove(v);
//         if removed {
//             self.vec.retain(|x| x!=v )
//         }
//         removed
//     }

//     pub fn get_index(&self, index: usize) -> Option<&T> {
//         self.vec.get(index)
//     }

//     // pub fn merge_set(&mut self, set: &mut HashSet<T>) {
//     //     for v in set.drain() {
//     //         self.insert(v);
//     //     }
//     // }

//     // fn remove_random(&mut self) -> T {
//     //     assert_eq!(self.set.len(), self.vec.len());
//     //     let index = thread_rng().gen_range(0, self.vec.len());
//     //     let elem = self.vec.swap_remove(index);
//     //     let was_present = self.set.remove(&elem);
//     //     assert!(was_present);
//     //     elem
//     // }
// }

