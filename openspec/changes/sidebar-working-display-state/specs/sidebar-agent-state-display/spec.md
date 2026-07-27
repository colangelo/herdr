# sidebar-agent-state-display

## ADDED Requirements

### Requirement: Aggregate row state is derived from a display ranking
The state a sidebar row displays for a group of panes SHALL be derived from a display
ranking in which an actively Working pane outranks a pane that is Idle and unseen ("done").
Blocked SHALL remain the highest-ranked state. This display ranking SHALL be distinct from
the attention ranking used for sorting and notifications, and both SHALL be defined in a
single shared location so they cannot drift apart.

#### Scenario: Working outranks done
- **WHEN** a workspace contains one pane whose agent finished while unseen (done) and one pane whose agent is actively working
- **THEN** the workspace row displays the working state

#### Scenario: Blocked still outranks working
- **WHEN** a workspace contains one blocked pane and one working pane
- **THEN** the workspace row displays the blocked state

#### Scenario: Done still outranks seen-idle
- **WHEN** a workspace contains one pane that finished while unseen and one pane that is idle and already seen
- **THEN** the workspace row displays the done state

#### Scenario: Attention ranking is unchanged
- **WHEN** panes are ranked for attention-ordered sorting or notification decisions
- **THEN** a pane that is Idle and unseen still ranks above a pane that is Working, as before this change

### Requirement: Displayed state is independent of sibling focus
The state a sidebar row displays SHALL be a function only of the agent states and seen
flags of the panes it covers. Focusing a pane SHALL NOT change the displayed state of a row
except through the state of the panes that row actually covers.

#### Scenario: Focusing a sibling does not reveal a working agent
- **WHEN** a workspace shows a working agent masked by nothing, and the user focuses a different pane in that workspace whose agent had finished while unseen
- **THEN** the workspace row displayed working both before and after the focus switch

#### Scenario: Working agent is visible without any interaction
- **WHEN** an agent starts working in a workspace the user is not currently viewing, alongside a sibling pane that finished while unseen
- **THEN** the workspace row displays the working state without the user switching to that workspace, tab, or pane

### Requirement: Sort order and motion target order agree
Any animated reordering of a sidebar list SHALL use the same ranking as the sort whose
result it animates toward, so that the motion target and the sorted order cannot disagree.

#### Scenario: Bubble motion settles on the sorted order
- **WHEN** bubble motion is enabled and a list containing a mix of blocked, done, working, idle, and unknown entries is sorted by priority
- **THEN** the motion target order equals the sorted order, and the animation settles instead of chasing a different target
