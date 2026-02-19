# UPSC Prelims Gamified Study App — Implementation Plan

## Context

Build a cross-platform (iOS-first) UPSC Prelims preparation app that gamifies the entire study experience using spaced repetition, mind maps, quizzes, PYQ practice, and a full achievement system. Google Sheets/Docs serve as the database.

**Content sources**: Free NCERT textbook PDFs (from official ncert.nic.in) + AI-generated content from Claude's knowledge of standard UPSC references (Laxmikanth, Spectrum, Shankar IAS level content). User can upload additional PDFs later for enrichment.

---

## Tech Stack

| Layer | Choice |
|---|---|
| Framework | React Native + Expo (SDK 52+, Expo Router) |
| Language | TypeScript |
| State | Zustand + MMKV persistence |
| Backend | Google Apps Script (deployed as web app) |
| Database | Google Sheets (structured data) + Google Docs (long-form notes) |
| Data reads | Google Sheets API (fast bulk reads) |
| Data writes | Apps Script REST endpoints (doGet/doPost) |
| Mind Maps | Custom SVG (`react-native-svg` + gesture handler + reanimated) |
| Charts | `victory-native` |
| Animations | `react-native-reanimated` + `lottie-react-native` |

---

## Google Sheets Schema (12 Sheets)

| Sheet | Purpose | Key Columns |
|---|---|---|
| `Subjects` | 7 GS subjects | subject_id, name, color, icon |
| `Topics` | All topics per subject | topic_id, subject_id, name, doc_id, mindmap_data, weightage |
| `Flashcards` | SR flashcard bank | card_id, topic_id, front, back, hint, difficulty |
| `PYQ_Questions` | Previous Year Qs (2010-2025) | pyq_id, year, question_text, options A-D, correct_answer, explanation, topic_id |
| `Quiz_Questions` | Practice MCQs | question_id, topic_id, question_text, options, correct_answer, explanation |
| `User_Profiles` | User data + XP/level/streak | user_id, display_name, total_xp, current_level, current_streak |
| `SR_Card_State` | Per-user per-card SM-2 state | user_id, card_id, easiness_factor, interval, repetitions, next_review_date |
| `Quiz_Attempts` | Quiz history | attempt_id, user_id, quiz_type, score, time_taken, xp_earned |
| `Daily_Challenges` | Curated daily quizzes | challenge_date, question_ids, xp_reward |
| `Leaderboard` | Weekly/monthly/all-time rankings | user_id, weekly_xp, monthly_xp, rank |
| `Mind_Map_Templates` | Mind map JSON per topic | mindmap_id, topic_id, nodes_json, edges_json |
| `XP_Transactions` | XP audit log | user_id, xp_amount, source, timestamp |

---

## App Navigation (5 Tabs)

```
[Home]       [Study]       [Practice]      [Progress]     [Profile]
Dashboard    Subject Grid   Practice Hub    Stats/Charts   Settings
Today's Plan  → Topics      SR Review       Achievements   Sync
Due Cards      → Notes      Daily Quiz      Leaderboard    Theme
Streak/XP      → Flashcards PYQ Browser
               → Mind Map   Mock Tests
               → Quiz       Weak Areas
```

---

## Core Features

### 1. Spaced Repetition (SM-2 Algorithm)
- Pure SM-2 implementation: quality 0-5 → updates EF, interval, repetitions
- 4-button rating UI: Again (red) / Hard (orange) / Good (blue) / Easy (green)
- Card scheduler: due cards sorted by overdue + difficulty
- Study sessions: interleave 2 due cards + 1 new card
- PYQ wrong answers auto-feed into SR system
- New cards per day: configurable (default 10)

### 2. Gamification Engine
- **XP System**: 5-50 XP per action (flashcard review, quiz, daily challenge, streaks)
- **Levels 1-50**: Quadratic curve (`100 * N^1.5` XP per level), titles from "Aspirant" to "UPSC Champion"
- **Streaks**: Daily tracking, freezes earned at milestones (7, 14, 30 days)
- **Badges**: ~25+ badges across categories (streak, subject mastery, quiz, PYQ, speed, level, special)
- **Leaderboard**: Weekly / Monthly / All-time XP rankings
- **Daily Challenges**: Curated 10-question quiz with bonus XP for perfect score

### 3. Mind Maps (Custom SVG)
- Read-only interactive maps per topic
- Radial tree layout (auto-computed or pre-positioned from Sheets)
- Pan/zoom via gesture handler + reanimated (60fps native thread)
- Tap node → bottom sheet with details + linked flashcards
- Data stored as JSON in Sheets

### 4. Quiz & PYQ Engine
- MCQ format with 4 options + detailed explanations
- PYQ bank: 2010-2025, filterable by year/subject/topic
- Mock tests: Full (100Q/2hr), Subject (25Q/30min), Quick (10Q/10min)
- Timed mode with countdown
- Results screen with per-question review
- Topic-wise accuracy analysis

---

## Apps Script API Design

- Single deployment URL, routes via `?action=` parameter
- **GET endpoints**: getSubjects, getTopics, getFlashcards, getMindMap, getPYQs, getDueCards, getLeaderboard, getDailyChallenge, getMockTest
- **POST endpoints**: createUser, submitReview, batchSubmitReviews, submitQuizAttempt, awardXP, checkBadges, updateStreak, syncUserData
- CORS avoided via `Content-Type: text/plain` POST + `redirect: follow`
- Quota-safe: aggressive local caching, batch requests, ~2-5 API calls per session

---

## Offline-First Strategy

- All content downloaded on first launch → cached in MMKV (~15-30 MB)
- User actions update local state immediately (optimistic)
- Writes queued in `pendingWrites` array, flushed when online
- Content versioning: single version number in Sheets, app polls on launch
- Network status detection via `@react-native-community/netinfo`

---

## Content Pipeline

### Sources
1. **NCERT Textbooks** (free, official): Download from ncert.nic.in
   - History: Class 6 (Our Pasts), 7, 8, 9, 10, 11, 12
   - Geography: Class 6-12 (including India: Physical Environment, Human Geography)
   - Political Science: Class 9-12 (Indian Constitution at Work, Political Theory)
   - Economics: Class 9-12 (Indian Economic Development)
   - Science: Class 6-10 (general science concepts)
2. **Claude's knowledge** for Laxmikanth-level Polity, Spectrum-level History, Shankar IAS-level Environment, standard Economy concepts
3. **User-provided PDFs** (future enrichment)

### Generation Flow
```
NCERT PDFs + Claude Knowledge
       │
       ▼
[Step 1] Claude generates structured content per topic
       │  - Notes (Markdown) → Google Docs (1 doc per topic)
       │  - Flashcard Q&A pairs → Flashcards sheet
       │  - Mind map JSON → Mind_Map_Templates sheet
       │  - MCQ questions → Quiz_Questions sheet
       │  - PYQs with explanations → PYQ_Questions sheet
       │
       ▼
[Step 2] Populate Google Sheets (bulk entry via Apps Script import function)
       │
       ▼
[Step 3] Manual verification pass (spot-check accuracy)
```

### Content priority: Polity → Economy → Geography → History → Environment → Science → Current Affairs

---

## Build Phases

### Phase 0: Foundation (Week 1-2)
- Expo project setup with Router + TypeScript
- Theme system, Zustand stores, MMKV persistence
- Google Sheets schema creation (all 12 sheets)
- Apps Script web app with router + deploy
- API client with cache-first fetching
- Subject Grid + Topic List screens reading from Sheets
- **Deliverable**: App displays subjects/topics from Google Sheets with offline cache

### Phase 1: Core Study Loop (Week 3-5)
- Flashcard flip component (Reanimated)
- SM-2 algorithm + card scheduler
- SR card state sync with Apps Script
- FlashcardDeck with swipe + quality rating
- Notes viewer (Markdown renderer for Google Docs)
- Topic Detail screen (Notes + Flashcards tabs)
- Basic XP for flashcard reviews
- **Deliverable**: Functional spaced repetition flashcards + notes reading + XP

### Phase 2: Quiz & PYQ Engine (Week 6-8)
- MCQ QuestionCard component
- Quiz session engine (queue, timer, scoring)
- Quiz Attempt + Results screens
- PYQ data entry (2020-2024) + browser UI
- Mock Test generator
- Daily Challenge system
- PYQ wrong answers → SR integration
- **Deliverable**: Complete quiz/PYQ practice with scoring and history

### Phase 3: Gamification (Week 9-10)
- Streak tracking + UI
- XP bar, level display, Level-Up modal (Lottie)
- Badge definitions + evaluator + unlock animations
- Achievements screen
- Leaderboard
- Home dashboard with Today's Plan
- **Deliverable**: Full gamification — streaks, XP, levels, badges, leaderboard

### Phase 4: Mind Maps (Week 11-13)
- SVG node/edge components
- Radial layout algorithm
- Pan/zoom canvas (gesture handler + reanimated)
- Node tap → detail bottom sheet
- Mind map data for all topics
- **Deliverable**: Interactive mind maps for all topics

### Phase 5: Polish (Week 14-16)
- Progress Dashboard (radar chart, heatmap, accuracy trends)
- Weak areas detection + auto-practice
- Push notifications (SR reminders, streak protection)
- Offline sync queue hardening
- Onboarding flow, app icon, splash screen
- EAS Build for TestFlight
- **Deliverable**: Production-ready app

---

## Project Structure

```
upsc-prelims-app/
├── app/                          # Expo Router (file-based)
│   ├── (auth)/                   # Login/register
│   ├── (tabs)/                   # 5-tab navigator
│   │   ├── home/                 # Dashboard
│   │   ├── study/                # Subjects → Topics → Detail
│   │   ├── practice/             # SR, Quiz, PYQ, Mock
│   │   ├── progress/             # Stats, Achievements, Leaderboard
│   │   └── profile/              # Settings
├── src/
│   ├── components/               # UI components
│   │   ├── ui/                   # Design system (Button, Card, XPBar, etc.)
│   │   ├── flashcard/            # FlashcardDeck, FlashcardItem, DifficultyRating
│   │   ├── mindmap/              # MindMapCanvas, MindMapNode, MindMapEdge
│   │   ├── quiz/                 # QuestionCard, OptionButton, ExplanationPanel
│   │   ├── charts/               # SubjectRadar, StreakCalendar, AccuracyTrend
│   │   └── gamification/         # LevelUpModal, BadgeUnlock, DailyReward
│   ├── features/                 # Business logic
│   │   ├── spaced-repetition/    # sm2Algorithm, cardScheduler
│   │   ├── gamification/         # xpEngine, levelSystem, streakTracker, badges
│   │   ├── mindmap/              # layout algorithm
│   │   ├── quiz/                 # quizEngine, mockTestGenerator, pyqService
│   │   └── content/              # contentService, subjectRegistry
│   ├── services/                 # API layer
│   │   ├── api/                  # client.ts, endpoints.ts
│   │   ├── sheetsApi.ts          # Direct Sheets reads
│   │   └── syncService.ts        # Offline sync queue
│   ├── store/                    # Zustand stores
│   ├── hooks/                    # useCachedData, useNetworkStatus, useTimer
│   └── theme/                    # colors, typography, spacing
├── google-apps-script/           # Apps Script source
│   ├── Code.gs                   # Main doGet/doPost router
│   ├── SheetsService.gs          # Sheet read/write helpers
│   ├── QuizService.gs            # Quiz/PYQ logic
│   ├── GamificationService.gs    # XP, badges, leaderboard
│   └── SRService.gs              # Spaced repetition server-side
├── assets/                       # Fonts, images, Lottie animations
└── app.config.ts                 # Expo configuration
```

---

## Verification Plan

1. **Phase 0**: Launch app on iOS simulator → see subjects grid fetched from Sheets → kill network → relaunch → data loads from cache
2. **Phase 1**: Review 10 flashcards → verify SM-2 scheduling in SR_Card_State sheet → verify XP incremented in User_Profiles sheet
3. **Phase 2**: Complete a PYQ set → verify attempt recorded → answer wrong → verify card appears in SR due queue next day
4. **Phase 3**: Earn enough XP to level up → verify modal appears → check badge unlock after streak milestone
5. **Phase 4**: Open mind map → pinch to zoom → tap node → verify details panel shows correct content
6. **Phase 5**: Go offline → complete quiz → go online → verify data syncs to Sheets
