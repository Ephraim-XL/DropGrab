// Sample content for the mock backend. Values come from the "Screens and flows"
// section of dropgrab-ui-design.md. Sizes are bytes (decimal MB, as the doc quotes them).

const MB = 1e6;
const GB = 1e9;

export const SELF = {
  id: 'dev-kofi-pixel8',
  name: "Kofi's Pixel 8",
  type: 'phone',
  os: 'Android',
  fingerprint: '4F3A 91C2 7D08 B6E1',
  visibility: 'everyone',
};

export const DEVICES_SEED = [
  {
    id: 'dev-maya-mba',
    name: "Maya's MacBook Air",
    type: 'laptop',
    os: 'Mac',
    trust: 'trusted',
    online: true,
    link: 'same Wi-Fi',
    lastSeen: 'today, 09:41',
    autoAccept: true,
    allowRemote: false,
    fingerprint: '7C1D 22A4 90EF 5B38',
  },
  {
    id: 'dev-office-pc',
    name: 'Office PC',
    type: 'desktop',
    os: 'Windows 11',
    trust: 'seen',
    online: true,
    link: 'same Wi-Fi',
    lastSeen: 'today, 09:12',
    autoAccept: false,
    allowRemote: true,
    fingerprint: 'A9E2 6B70 41C5 D8F3',
  },
  {
    id: 'dev-ama-iphone',
    name: "Ama's iPhone 15",
    type: 'phone',
    os: 'iPhone',
    trust: 'trusted',
    online: true,
    link: 'same Wi-Fi',
    lastSeen: 'yesterday, 18:20',
    autoAccept: false,
    allowRemote: false,
    fingerprint: '31B8 5F02 E7A1 6C9D',
  },
  {
    id: 'dev-nana-tab',
    name: "Nana's Galaxy Tab",
    type: 'tablet',
    os: 'Android tablet',
    trust: 'trusted',
    online: false,
    link: 'last seen Monday',
    lastSeen: 'Monday, 20:05',
    autoAccept: false,
    allowRemote: false,
    fingerprint: 'D4C0 18BB 2E76 04A5',
  },
  {
    id: 'dev-living-room',
    name: 'Living Room Mini PC',
    type: 'desktop',
    os: 'Linux',
    trust: 'seen',
    online: false,
    link: 'last seen Sunday',
    lastSeen: 'Sunday, 11:30',
    autoAccept: false,
    allowRemote: true,
    fingerprint: '6E77 90D1 C3B4 8F12',
  },
];

export const FILES = {
  thesis: { id: 'f-thesis', name: 'Thesis draft v3.docx', kind: 'document', size: 1.2 * MB },
  figures: { id: 'f-figures', name: 'Chapter 4 figures.zip', kind: 'archive', size: 48.6 * MB },
  defense: { id: 'f-defense', name: 'Defense rehearsal.mp4', kind: 'video', size: 612 * MB },
  lecture: { id: 'f-lecture', name: 'Lecture 07 Operating Systems.mp4', kind: 'video', size: 734 * MB },
  img: { id: 'f-img', name: 'IMG_4821.HEIC', kind: 'image', size: 3.4 * MB },
  iso: { id: 'f-iso', name: 'ubuntu-24.04.2-desktop-amd64.iso', kind: 'app', size: 5.9 * GB },
  podcast: { id: 'f-podcast', name: 'Podcast Episode 112.mp3', kind: 'audio', size: 58 * MB },
  annual: { id: 'f-annual', name: 'Annual report 2025.pdf', kind: 'document', size: 4.1 * MB },
  rust: { id: 'f-rust', name: 'Rust ownership explained.mp4', kind: 'video', size: 412 * MB },
};

// Pre-built batches behind the "Choose what to send" sheet rows.
export const SEND_BATCHES = {
  photos: { label: 'Photos and videos', files: [FILES.lecture, FILES.img, FILES.defense] },
  documents: { label: 'Documents', files: [FILES.thesis, FILES.annual] },
  folders: { label: 'Folders', files: [FILES.figures] },
  clipboard: { label: 'Text from the clipboard', text: 'https://example.org/lectures/operating-systems.pdf' },
  library: { label: 'Files already in DropGrab', files: [FILES.lecture, FILES.podcast] },
};

export const INCOMING_FILES = [FILES.thesis, FILES.figures, FILES.defense];

export const DOWNLOADS_SEED = [
  {
    id: 'job-rust',
    name: 'Rust ownership explained.mp4',
    kind: 'video',
    state: 'downloading',
    url: 'https://video.example.org/watch/rust-ownership',
    total: 412 * MB,
    done: 412 * MB * 0.63,
    bps: 8.2 * MB,
    quality: '1080p · MP4',
    startedAt: Date.now() - 61000,
  },
  {
    id: 'job-podcast',
    name: 'Podcast Episode 112.mp3',
    kind: 'audio',
    state: 'downloading',
    url: 'https://music.example.org/episode/112',
    total: 58 * MB,
    done: 58 * MB * 0.27,
    bps: 5.4 * MB,
    quality: 'M4A · 192 kbps',
    startedAt: Date.now() - 9000,
  },
  {
    id: 'job-ubuntu',
    name: 'ubuntu-24.04.2-desktop-amd64.iso',
    kind: 'app',
    state: 'paused',
    url: 'https://releases.example.org/ubuntu-24.04.2-desktop-amd64.iso',
    total: 5.9 * GB,
    done: 2.4 * GB,
    bps: 6.1 * MB,
    quality: 'Direct file',
    startedAt: Date.now() - 3600000,
  },
  {
    id: 'job-annual',
    name: 'Annual report 2025.pdf',
    kind: 'document',
    state: 'finished',
    url: 'https://example.org/files/annual-report-2025.pdf',
    total: 4.1 * MB,
    done: 4.1 * MB,
    bps: 0,
    quality: 'Direct file',
    finishedAt: Date.now() - 5400000,
  },
];

export const LIBRARY_SEED = [
  {
    id: 'itm-lecture',
    name: FILES.lecture.name,
    kind: 'video',
    size: FILES.lecture.size,
    origin: 'download',
    source: 'Downloaded from example.org',
    when: '3 days ago',
  },
  {
    id: 'itm-thesis',
    name: FILES.thesis.name,
    kind: 'document',
    size: FILES.thesis.size,
    origin: 'received',
    source: "Received from Ama's iPhone 15",
    when: 'yesterday',
  },
  {
    id: 'itm-img',
    name: FILES.img.name,
    kind: 'image',
    size: FILES.img.size,
    origin: 'received',
    source: "Received from Maya's MacBook Air",
    when: 'Monday',
  },
  {
    id: 'itm-annual',
    name: FILES.annual.name,
    kind: 'document',
    size: FILES.annual.size,
    origin: 'download',
    source: 'Downloaded from example.org',
    when: 'today',
  },
  {
    id: 'itm-podcast',
    name: FILES.podcast.name,
    kind: 'audio',
    size: FILES.podcast.size,
    origin: 'download',
    source: 'Downloaded from music.example.org',
    when: 'today',
  },
  {
    id: 'itm-figures',
    name: FILES.figures.name,
    kind: 'archive',
    size: FILES.figures.size,
    origin: 'received',
    source: "Received from Maya's MacBook Air",
    when: '2 days ago',
  },
];

export const RECENT_SEED = [
  {
    id: 'rec-lecture',
    name: FILES.lecture.name,
    kind: 'video',
    direction: 'sent',
    peer: "Maya's MacBook Air",
    size: FILES.lecture.size,
    when: '2 min ago',
  },
  {
    id: 'rec-thesis',
    name: FILES.thesis.name,
    kind: 'document',
    direction: 'received',
    peer: "Ama's iPhone 15",
    size: FILES.thesis.size,
    when: 'yesterday',
  },
];

export const QUALITY_OPTIONS = [
  { id: 'best', label: 'Best', detail: '1080p MP4', size: 412 * MB },
  { id: 'balanced', label: 'Balanced', detail: '720p MP4', size: 198 * MB },
  { id: 'data-saver', label: 'Data saver', detail: '480p MP4', size: 96 * MB },
  { id: 'audio', label: 'Audio only', detail: 'M4A', size: 17 * MB },
];

export const DETECTED_ITEMS = [
  {
    id: 'det-lecture-stream',
    name: 'Lecture 07 Operating Systems',
    kind: 'video',
    duration: '1:12:08',
    quality: '1080p',
    size: 734 * MB,
    formats: QUALITY_OPTIONS,
  },
  {
    id: 'det-slides',
    name: 'Chapter 4 slides',
    kind: 'document',
    duration: 'PDF',
    quality: 'Original',
    size: 12.4 * MB,
    formats: [],
  },
];

export const SETTINGS_DEFAULT = {
  deviceName: SELF.name,
  visibility: 'everyone',
  saveLocation: 'Received',
  askBeforeAccepting: true,
  downloadFolder: 'DropGrab/Downloads',
  downloadsAtOnce: 3,
  wifiOnly: true,
  speedLimit: 'off',
  defaultQuality: 'balanced',
  audioFormat: 'M4A',
  theme: 'system',
  textSize: 'default',
  privateMode: false,
  hideLockDetails: false,
  version: '1.0.0 (build 214)',
};
