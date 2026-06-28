import { useEffect, useState } from 'react';
import { useLibraryStore } from '../store/library';

export function useLibrary() {
  const store = useLibraryStore();
  const [search, setSearch] = useState('');

  useEffect(() => {
    store.loadAll();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // debounced search
  useEffect(() => {
    const t = setTimeout(() => {
      store.loadTracks(search || undefined);
    }, 250);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [search]);

  return { ...store, search, setSearch };
}
