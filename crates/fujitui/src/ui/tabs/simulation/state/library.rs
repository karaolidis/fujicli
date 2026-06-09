use std::collections::BTreeMap;

use fujicore::features::simulation::SimulationDescriptors;

use crate::{
    ui::tabs::Buffer,
    workers::fs::{
        simulation::{SimulationLibraryEntry, SimulationLibrarySnapshot},
        slug::Slug,
    },
};

use super::SimulationState;

#[derive(Debug, Clone)]
pub(super) struct SimulationLibraryBuffer {
    pub entry: SimulationLibraryEntry,
    pub buffer: Buffer<SimulationState>,
    pub descriptors: &'static SimulationDescriptors,
}

#[derive(Debug, Default)]
pub(in crate::ui::tabs::simulation) struct SimulationLibrarySyncReport {
    pub added: Vec<Slug>,
    pub removed: Vec<Slug>,
    pub updated: Vec<Slug>,
    pub updated_with_conflict: Vec<Slug>,
    pub unsupported: Vec<Slug>,
}

#[derive(Debug, Default)]
pub(super) struct SimulationLibraryView {
    pub(super) entries: BTreeMap<Slug, SimulationLibraryBuffer>,
}

impl SimulationLibraryView {
    pub fn sync(&mut self, snapshot: &SimulationLibrarySnapshot) -> SimulationLibrarySyncReport {
        let mut report = SimulationLibrarySyncReport::default();

        let removed: Vec<Slug> = self
            .entries
            .keys()
            .filter(|slug| !snapshot.entries.contains_key(*slug))
            .cloned()
            .collect();
        for slug in &removed {
            self.entries.remove(slug);
        }
        report.removed = removed;

        for (slug, entry) in &snapshot.entries {
            let Some(descriptors) = entry
                .source_camera
                .supported_camera()
                .and_then(|c| c.simulation)
            else {
                if self.entries.remove(slug).is_some() {
                    report.removed.push(slug.clone());
                }
                report.unsupported.push(slug.clone());
                continue;
            };

            let state = SimulationState {
                canonical: entry.simulation.clone(),
                shadow: descriptors.new_shadow_from(&entry.simulation),
            };
            match self.entries.get_mut(slug) {
                None => {
                    self.entries.insert(
                        slug.clone(),
                        SimulationLibraryBuffer {
                            entry: entry.clone(),
                            buffer: Buffer {
                                fetched: state.clone(),
                                working: state,
                            },
                            descriptors,
                        },
                    );
                    report.added.push(slug.clone());
                }
                Some(lib) if lib.buffer.fetched.canonical == entry.simulation => {
                    lib.entry = entry.clone();
                }
                Some(lib) => {
                    let has_unsaved_edits = lib.buffer.dirty();
                    lib.entry = entry.clone();
                    lib.buffer = Buffer {
                        fetched: state.clone(),
                        working: state,
                    };
                    if has_unsaved_edits {
                        report.updated_with_conflict.push(slug.clone());
                    } else {
                        report.updated.push(slug.clone());
                    }
                }
            }
        }

        report
    }

    pub fn get(&self, slug: &Slug) -> Option<&SimulationLibraryBuffer> {
        self.entries.get(slug)
    }

    pub fn get_mut(&mut self, slug: &Slug) -> Option<&mut SimulationLibraryBuffer> {
        self.entries.get_mut(slug)
    }
}

impl<'a> IntoIterator for &'a SimulationLibraryView {
    type Item = (&'a Slug, &'a SimulationLibraryBuffer);
    type IntoIter = std::collections::btree_map::Iter<'a, Slug, SimulationLibraryBuffer>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use fujicore::{
        UsbId,
        generated::{options::FilmSimulation, simulations::SimulationBase},
    };
    use time::OffsetDateTime;

    use super::*;

    fn snapshot_with(entries: Vec<(Slug, SimulationLibraryEntry)>) -> SimulationLibrarySnapshot {
        let mut s = SimulationLibrarySnapshot::default();
        for (k, v) in entries {
            s.entries.insert(k, v);
        }
        s
    }

    fn sample_entry(name: &str, sim: SimulationBase) -> SimulationLibraryEntry {
        let now = OffsetDateTime::now_utc();
        SimulationLibraryEntry {
            name: name.to_owned(),
            source_camera: UsbId {
                vendor: 0x04CB,
                product: 0x02FC,
            },
            created: now,
            modified: now,
            simulation: sim,
        }
    }

    #[test]
    fn sync_library_preserves_unchanged_entries() {
        let mut lib = SimulationLibraryView::default();
        let slug = Slug::try_from("entry-a").unwrap();
        let sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        let entry = sample_entry("Entry A", sim);
        let first = lib.sync(&snapshot_with(vec![(slug.clone(), entry.clone())]));
        assert_eq!(first.added, vec![slug.clone()]);
        let second = lib.sync(&snapshot_with(vec![(slug, entry)]));
        assert!(second.added.is_empty());
        assert!(second.removed.is_empty());
        assert!(second.updated.is_empty());
        assert!(second.updated_with_conflict.is_empty());
    }

    #[test]
    fn sync_library_flags_conflict_when_user_has_edits() {
        let mut lib = SimulationLibraryView::default();
        let slug = Slug::try_from("entry-a").unwrap();
        lib.sync(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", SimulationBase::default()),
        )]));

        lib.entries.get_mut(&slug).unwrap().buffer.working.canonical = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        assert!(lib.entries[&slug].buffer.dirty());

        let external_sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Astia),
            ..Default::default()
        };
        let report = lib.sync(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", external_sim),
        )]));
        assert_eq!(report.updated_with_conflict, vec![slug]);
    }

    #[test]
    fn library_buffer_dirty_is_false_after_sync() {
        let mut lib = SimulationLibraryView::default();
        let slug = Slug::try_from("entry-a").unwrap();
        let sim = SimulationBase {
            film_simulation: Some(FilmSimulation::Velvia),
            ..Default::default()
        };
        lib.sync(&snapshot_with(vec![(
            slug.clone(),
            sample_entry("Entry A", sim),
        )]));
        let entry = lib.entries.get(&slug).unwrap();
        assert!(!entry.buffer.dirty());
    }
}
