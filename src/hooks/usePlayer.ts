import { useEffect, useRef } from 'react';
import { usePlayerStore } from '../store/player';
import { api } from '../lib/api';

/**
 * Singleton-ish hook that owns the hidden <audio> element and bridges
 * between the persisted Zustand player store and the actual HTMLMediaElement.
 * Mount this once near the root of the app.
 *
 * Ownership model:
 *  - audio.currentTime is normally driven by the element's `timeupdate` event.
 *  - Store-initiated seeks (seek(), prev()-restart, repeat-one) can't reach the
 *    element from the store, so they set `pendingSeek`, which an effect here
 *    applies to the element and then clears.
 */
export function usePlayer() {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  if (audioRef.current === null && typeof Audio !== 'undefined') {
    audioRef.current = new Audio();
    audioRef.current.preload = 'metadata';
  }

  // Subscribe only to the fields the bridge reads/writes — NOT the whole store.
  // Subscribing to the whole store would re-render the whole app ~4x/second via
  // timeupdate -> setCurrentTime -> currentTime change.
  const queue = usePlayerStore((s) => s.queue);
  const currentIndex = usePlayerStore((s) => s.currentIndex);
  const isPlaying = usePlayerStore((s) => s.isPlaying);
  const volume = usePlayerStore((s) => s.volume);
  const muted = usePlayerStore((s) => s.muted);
  const pendingSeek = usePlayerStore((s) => s.pendingSeek);
  const next = usePlayerStore((s) => s.next);
  const setCurrentTime = usePlayerStore((s) => s.setCurrentTime);
  const setDuration = usePlayerStore((s) => s.setDuration);

  const currentTrack = queue[currentIndex];

  // track which track id we've already counted a play for, so rapid skips don't
  // inflate play counts on the server.
  const countedPlayRef = useRef<number | null>(null);

  // load source when track changes
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !currentTrack) {
      // queue emptied — fully halt playback (removing src without pause/load can
      // leave an orphaned track playing).
      if (audio) {
        audio.pause();
        audio.removeAttribute('src');
        audio.load();
      }
      return;
    }
    audio.src = api.streamUrl(currentTrack.id);
    audio.load();
    countedPlayRef.current = null;
    if (isPlaying) {
      audio
        .play()
        .then(() => {
          countedPlayRef.current = currentTrack.id;
          api.markPlayed(currentTrack.id).catch(() => {});
        })
        .catch(() => {
          // autoplay rejection — keep store paused
          usePlayerStore.setState({ isPlaying: false });
        });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentTrack?.id]);

  // play/pause
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !currentTrack) return;
    if (isPlaying) {
      audio
        .play()
        .then(() => {
          if (countedPlayRef.current !== currentTrack.id) {
            countedPlayRef.current = currentTrack.id;
            api.markPlayed(currentTrack.id).catch(() => {});
          }
        })
        .catch(() => {
          usePlayerStore.setState({ isPlaying: false });
        });
    } else {
      audio.pause();
    }
  }, [isPlaying, currentTrack]);

  // volume
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.volume = volume;
    audio.muted = muted;
  }, [volume, muted]);

  // media element events
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    // guard against rapid infinite loops when many tracks in a row are unplayable
    let consecutiveErrors = 0;

    const onTime = () => setCurrentTime(audio.currentTime);
    const onDur = () => {
      if (Number.isFinite(audio.duration)) setDuration(audio.duration);
    };
    const onPlaying = () => {
      consecutiveErrors = 0;
    };
    const onEnd = () => {
      // repeat-one must restart the same element; the store's next() can't reach
      // the audio (the track id and isPlaying don't change, so neither load-source
      // nor play/pause effects re-run).
      const { repeat } = usePlayerStore.getState();
      if (repeat === 'one') {
        audio.currentTime = 0;
        setCurrentTime(0);
        audio.play().catch(() => usePlayerStore.setState({ isPlaying: false }));
        return;
      }
      next();
    };
    const onError = () => {
      consecutiveErrors += 1;
      const { queue, repeat } = usePlayerStore.getState();
      // break the cycle: stop after enough consecutive failures to have wrapped
      // the queue (or a sane cap when not repeating).
      const limit = repeat === 'all' ? queue.length : 3;
      if (consecutiveErrors > limit) {
        usePlayerStore.setState({ isPlaying: false });
        return;
      }
      next();
    };

    audio.addEventListener('timeupdate', onTime);
    audio.addEventListener('durationchange', onDur);
    audio.addEventListener('playing', onPlaying);
    audio.addEventListener('ended', onEnd);
    audio.addEventListener('error', onError);
    return () => {
      audio.removeEventListener('timeupdate', onTime);
      audio.removeEventListener('durationchange', onDur);
      audio.removeEventListener('playing', onPlaying);
      audio.removeEventListener('ended', onEnd);
      audio.removeEventListener('error', onError);
    };
  }, [next, setCurrentTime, setDuration]);

  // consume pendingSeek: apply store-driven seeks to the audio element
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || pendingSeek == null) return;
    const t = pendingSeek;
    // clear the intent first so a subsequent setCurrentTime doesn't retrigger
    usePlayerStore.setState({ pendingSeek: null });
    if (Number.isFinite(t) && t >= 0) {
      audio.currentTime = t;
      setCurrentTime(t);
    }
  }, [pendingSeek, setCurrentTime]);

  // expose imperatively for the NowPlayingBar to seek
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    (usePlayerStore.getState() as unknown as { _seekAudio?: (t: number) => void })._seekAudio = (
      t: number,
    ) => {
      if (Number.isFinite(t)) {
        audio.currentTime = t;
        setCurrentTime(t);
      }
    };
  }, [setCurrentTime]);

  // unmount cleanup: pause + release the audio element (guards against StrictMode
  // double-mount in dev leaving an orphaned element playing).
  useEffect(() => {
    const audio = audioRef.current;
    return () => {
      if (!audio) return;
      audio.pause();
      audio.removeAttribute('src');
      audio.load();
      delete (usePlayerStore.getState() as unknown as { _seekAudio?: (t: number) => void })._seekAudio;
    };
  }, []);

  return { audioRef, currentTrack };
}
