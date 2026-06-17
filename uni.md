## 0. 三种候选架构比较与主方案选择

| 候选架构                                                     | 核心思想                                                     | 强项                                                         | 致命问题                                                     | 适合承担的层                                  |
| ------------------------------------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ | --------------------------------------------- |
| **1. 每个恒星系一条本地 BFT ledger + 跨星系 checkpoint**     | 每个恒星系/信任域内部维护本地可终结账本；跨星系只交换签名 checkpoint、证明和回执 | 工程上最可行；本地稀缺资产可串行化；证明短；客户端可做 light verification；本地 finality 清晰 | 跨星系没有实时全局状态；远方只能多年后知道本地结果；如果本地委员会作恶，远方只能事后追责 | **主账本层**                                  |
| **2. 全局 causal DAG ledger，无全局 total order**            | 所有事件形成全银河因果 DAG；只有 happens-before，没有全局排序 | 最符合相对论因果结构；天然支持离线、乱序、异步传播；不会假装有全局时钟 | 稀缺资产需要冲突裁决；DAG 越来越大；没有统一 finality；钱包 UX 极难；双花检测只能“发现”，不能实时“阻止” | **证据层 / 因果图层**，不适合作为唯一结算账本 |
| **3. credit-network / clearing-network 优先，账本只记录债权和证明** | 不试图跨光年即时转移资产，而是记录债权、信用额度、抵押、回执和违约证明 | 最符合星际贸易现实；可用信用额度限制损失；可折价承兑；能表达风险 | 需要债权根源、抵押和治理；不能单靠密码学保证清偿；如果没有底层资产锚定，容易变成纯信用账 | **跨星系清算层 / 风险管理层**                 |

**主方案选择：采用 1 为硬账本层，3 为跨星系清算层，2 退化为因果证据层。**

也就是：

> **每个恒星系维护本地 BFT ledger；跨恒星系只传播签名 checkpoint、锁定证明、债权凭证和回执；全银河不做 total order，只维护一个 causal checkpoint mesh；远程承兑永远显式标注信用风险和 settlement horizon。**

这个选择的理由很简单：
本地稀缺资产必须有某个局部 total order，否则无法在本地防止双花；跨星系 total order 在物理上不可实现；跨星系商业只能通过债权、信用额度、抵押和长期清算来完成。

------

# A. 一句话设计哲学

**不要构造一个幻想中的全银河区块链；要构造一个遵守光锥因果约束的“局部强一致账本 + 异步债权清算网络”。**

## 核心目标

系统解决的是：

1. **本地资产的确定性归属**
   - 在一个恒星系或本地信任域内，资产、合约、抵押和债权有明确状态。
   - 本地 ledger 在本地 BFT 假设下提供强 finality。
2. **跨星系资产锁定证明**
   - A 星系可以证明某资产已经在 A 本地 ledger 中被锁定、冻结或承诺给远方。
   - B 星系可以多年后验证这个证明。
3. **跨星系债权、远期合同和清算凭证**
   - 远方不是立即拥有原资产，而是获得一个可验证、可折价、可争议、可清算的 claim。
4. **显式风险表达**
   - 客户端必须告诉用户：这是本地最终、远方已观察、临时承兑、双边清算，还是仍处于风险窗口中。
   - 不能把“暂时没看到冲突”显示成“全银河确认”。
5. **长期自治**
   - 某个恒星系即使与其他恒星系失联数十年，仍可继续本地运行。
   - 重新连接时，通过 checkpoint、causal certificate 和 dispute 机制合并历史。

## 非目标

系统不解决：

1. **全银河即时一致性**
   - 不存在超光速通信。
   - 全局共识轮次至少需要跨星系往返时间，可能是十几年到几十年。
   - 对日常支付不可用，对高频交易更不可用。
2. **完全无信用的跨星系即时支付**
   - B 星系在未收到 A 星系最新状态前，不可能密码学地确认 A 没有在别处产生冲突声明。
   - 只能通过信用额度、抵押、保险池、折价承兑来管理风险。
3. **防止本地政权或本地清算委员会集体作恶**
   - 密码学能证明他们作恶。
   - 密码学不能强迫他们赔偿。
   - 赔偿依赖抵押、声誉、治理、法律和军事/政治现实。
4. **证明某个远方事实不存在**
   - 在光速限制下，“我还没收到冲突”不是“冲突不存在”。
   - 客户端只能说：“在我的当前光锥内，未观察到冲突。”

## 为什么不能追求全局即时一致性

设 Earth 与 Alpha Centauri 相距 4.3 光年。
任何从 Earth 发出的状态变更，Alpha Centauri 最早也要 4.3 年后才能知道。若要求 Earth 和 Alpha Centauri 对每一笔交易达成全局共识，单轮确认至少需要一个或多个跨星系传播延迟。若涉及更多恒星系，延迟会增长到几十年。

因此，全局即时一致性不是工程上困难，而是物理上不可定义。

## 在光速限制下重新定义 finality

本系统不使用单一 finality，而使用**观察者相关的 finality**：

| Finality 类型                      | 含义                                       | 强保证还是风险管理                                       |
| ---------------------------------- | ------------------------------------------ | -------------------------------------------------------- |
| `local-finality`                   | 交易已被本地 BFT ledger 终结               | 在本地 BFT 假设下是强保证                                |
| `remote-observed-finality`         | 远方已经收到并验证某本地 checkpoint        | 强保证：证明该 checkpoint 存在；但不证明没有其他远方冲突 |
| `provisional-credit`               | 远方基于 claim、信用额度、抵押进行临时承兑 | 风险管理                                                 |
| `interstellar-settlement-finality` | 双方 ledger 都记录了锁定、承兑、回执和确认 | 强保证 + 风险管理混合                                    |
| `bilateral-finality`               | 双方根据协议、挑战期、治理规则认为结算完成 | 主要是治理和经济 finality                                |
| `global-finality`                  | 全银河所有可能观察者都不可再发现冲突       | 本系统不承诺                                             |

------

# B. 物理 / 因果模型

## B.1 实体模型

系统中的实体分为：

```text
StarSystem
  ├── LocalLedgerDomain
  ├── ValidatorCommittee
  ├── ClearingNode
  ├── ArchiveNode
  ├── RelayNode
  ├── WalletClient
  ├── AI Financial Agent
  └── Local Observatories / Time Authorities

Planet / Habitat / Station
  ├── 普通钱包
  ├── 本地清算节点
  ├── 本地验证节点
  └── 本地归档节点

Ship
  ├── shipborne relay
  ├── offline wallet
  ├── delayed archive carrier
  └── moving worldline observer
```

一个“恒星系”不是纯粹天文学概念，而是一个**ledger trust domain**。
它可以是：

- 一个恒星系；
- 一个行星联盟；
- 一个空间站集群；
- 一支长期航行舰队；
- 一个本地清算联盟。

为了简单，主设计假设：

> 每个主要恒星系有一个 canonical local ledger domain。
> 更复杂的本地分片可以在后续扩展中加入。

## B.2 Spacetime coordinate 与 worldline

每个节点 `n` 有一条 worldline：

```text
worldline_n : proper_time -> spacetime_coordinate
```

一个 spacetime coordinate 包含：

```text
coord = {
  frame_id: ReferenceFrame,
  time_interval: [t_min, t_max],
  position_region: Region3D,
  uncertainty: ε,
  attestation: signatures from local observatories / beacons
}
```

不要求宇宙中存在完美全局时钟。
但协议需要一个**可审计的参考坐标系**，例如某个星际导航参考框架。实际工程中所有坐标都带不确定性区间。

## B.3 Event

事件是账本系统中最小的因果对象：

```text
Event e = {
  event_id,
  actor_id,
  domain_id,
  kind,
  payload_hash,
  coord,
  local_sequence,
  signatures
}
```

事件可以是：

- 本地交易提交；
- 本地交易 commit；
- checkpoint 生成；
- checkpoint 被某节点观察；
- claim 被接收；
- claim 被承兑；
- dispute 被打开；
- relay 转发；
- ship reconnect。

## B.4 Message

消息是事件之间的因果边：

```text
Message m = {
  msg_id,
  from_event,
  to_event,
  payload_hash,
  route,
  send_coord,
  receive_coord,
  relay_signatures,
  anti_replay_nonce
}
```

消息只能沿未来光锥传播。

使用单位 `c = 1 lightyear/year` 时，若事件 `e1` 的坐标为 `(t1, x1)`，事件 `e2` 的坐标为 `(t2, x2)`，则：

```text
e1 <=_lightcone e2
iff
t2 >= t1
and
t2 - t1 >= distance(x1, x2)
```

带不确定性时使用保守判断：

```text
def lightcone_possible(e1, e2):
    return e2.t_min - e1.t_max >= min_distance(e1.region, e2.region) - epsilon
```

若该条件不成立，客户端不得接受“e2 已经知道 e1”的证明。

## B.5 Observation

Observation 是“某节点在某事件中观察到了某数据”的签名声明：

```text
Observation o = {
  observer_id,
  observed_hash,
  observed_kind,
  receive_event,
  receive_coord,
  source_route,
  signature
}
```

Observation 不等于事实本身。
它只说明：

> 观察者声称在某个 spacetime event 收到了某个 payload。

客户端必须验证：

1. payload hash 正确；
2. observation 签名正确；
3. observation 的接收事件在物理上可能收到该 payload；
4. 该 observation 被本地 ledger 或 checkpoint 纳入。

## B.6 Local ledger state

一个本地 ledger domain `D` 的状态：

```text
S_D(h) = Apply(S_D(h-1), Block_D(h))
```

其中 `h` 是本地 ledger height。
本地状态只需要在 `D` 内部 total order。
不同恒星系之间不建立 total order。

## B.7 Checkpoint

Checkpoint 是本地 ledger 对某个高度状态的短证明：

```text
Checkpoint C_D,h = {
  domain_id: D,
  height: h,
  epoch,
  prev_checkpoint_hash,
  state_root,
  tx_root,
  event_log_root,
  export_root,
  import_root,
  observed_remote_root,
  dispute_root,
  validator_set_root,
  timestamp_interval,
  spatial_bound,
  quorum_certificate,
  protocol_version
}
```

一个 checkpoint 是跨星系传播的主要对象。
远方客户端通常不下载完整账本，只验证 checkpoint、Merkle proof、validator set proof 和 causal certificate。

## B.8 Causal dependency

事件 `a` 是事件 `b` 的因果依赖，当且仅当：

1. `b` 的 payload 显式引用 `a`；
2. 有消息从 `a` 或其后继传递到 `b`；
3. `a` 和 `b` 在同一本地 ledger 中，且 `a` 的 ledger sequence 早于 `b`；
4. `b` 的 checkpoint 包含了对 `a` 所在 checkpoint 的 observation；
5. `b` 的 claim / receipt / dispute 证明引用了 `a`。

记作：

```text
a -> b
```

其传递闭包为：

```text
a happens-before b
```

## B.9 Lightcone-validity

一个证明 `P` 对观察者事件 `r` 是 lightcone-valid，当且仅当：

```text
for every dependency event e in P:
    exists path e = v0 -> v1 -> ... -> vk = r
    such that every hop vi -> vi+1 is lightcone_possible
```

客户端强制规则：

> 不接受任何声称包含“当前事件不可能已经知道的信息”的证明。

这是一条强安全规则。
但它依赖物理坐标 attestation 的质量。坐标 attestation 本身有治理和测量假设。

## B.10 Settlement horizon

对从 A 星系到 B 星系的 claim，最早时间大致是：

```text
t0 = A 锁定资产时间
dAB = A 与 B 的光程距离

B 最早观察 A lock:
  t0 + dAB

A 最早观察 B acceptance:
  t0 + 2*dAB + local_commit_delays

B 最早观察 A acknowledgement:
  t0 + 3*dAB + local_commit_delays
```

但真正的 settlement horizon 还要考虑：

- 其他可能冲突接收方；
- watcher 节点分布；
- challenge window；
- checkpoint 间隔；
- relay 审查风险；
- 本地委员会 slashing 规则；
- 保险池等待期。

定义：

```text
settlement_horizon_B(claim) =
  max over relevant watcher domains W:
      earliest_time_B_can_receive_conflict_evidence_from(W)
  + challenge_window
  + checkpoint_grace_period
```

关键点：

> settlement horizon 不是一个全局常数，而是和观察者、资产归属域、通信拓扑、风险策略相关。

## B.11 同步、部分同步、异步模型适用层级

| 层级                    | 模型                               | 说明                                               |
| ----------------------- | ---------------------------------- | -------------------------------------------------- |
| 单机器客户端内部        | 同步 / deterministic state machine | 可完全测试                                         |
| 同一空间站 / 行星局域网 | 部分同步                           | 可使用常规 BFT 或 crash fault tolerant replication |
| 同一恒星系              | 部分同步，延迟有上界估计           | 适合 BFT ledger                                    |
| 恒星系之间              | 异步 + 已知光速下界，无可靠上界    | 不适合共识，只适合 checkpoint exchange             |
| 飞船 / 深空 relay       | delay-tolerant networking          | 数据包可能几十年后到达                             |

## B.12 CAP、FLP、BFT 在星际场景下的含义

**CAP：**

跨星系 partition 是常态，不是异常。
如果要求全局 consistency，就牺牲跨星系 availability，系统会停摆多年。
因此本设计选择：

```text
本地：Consistency + Availability within local assumptions
全局：Partition-tolerance + explicit risk, no global consistency
```

**FLP：**

在完全异步网络中，确定性共识无法同时保证 safety 和 liveness。
星际网络本质上是异步的，因此不能在全银河层做活性有保证的共识。

**BFT：**

BFT 可以用于本地恒星系，因为本地网络可以近似部分同步。
跨星系 BFT 没有工程意义，因为一轮共识消息传播就可能消耗多年。

------

# C. 系统架构总览

主架构名称：

```text
LCCM: Local BFT Ledgers + Causal Checkpoint Mesh + Credit Clearing Overlay
```

## C.1 分层结构

```text
┌────────────────────────────────────────────┐
│ User / AI financial agent                  │
├────────────────────────────────────────────┤
│ Client state machine / risk UI             │
├────────────────────────────────────────────┤
│ Settlement protocol                        │
│ - claim                                    │
│ - receipt                                  │
│ - dispute                                  │
│ - slashing evidence                        │
├────────────────────────────────────────────┤
│ Credit / clearing network                  │
│ - credit line                              │
│ - collateral                               │
│ - insurance pool                           │
│ - haircut                                  │
├────────────────────────────────────────────┤
│ Causal checkpoint mesh                     │
│ - signed checkpoints                       │
│ - observed remote checkpoints              │
│ - causal certificate                       │
│ - lightcone proof                          │
├────────────────────────────────────────────┤
│ Local BFT ledger per trust domain          │
│ - local total order                        │
│ - local finality                           │
│ - asset locks                              │
│ - contract state                           │
├────────────────────────────────────────────┤
│ Physical message layer                     │
│ - laser / radio / neutrino / shipborne DTN │
│ - delay / replay / drop / censorship       │
└────────────────────────────────────────────┘
```

## C.2 Local ledger domain

每个 ledger domain `D` 有：

```text
Domain D = {
  domain_id,
  spatial_bounds,
  genesis_checkpoint,
  validator_epoch_schedule,
  local_consensus_protocol,
  asset_registry,
  clearing_policy,
  slashing_policy,
  accepted_crypto_suites
}
```

本地 ledger 负责：

- 本地账户状态；
- 本地资产对象；
- 本地合约；
- 本地抵押；
- 出口锁定；
- 进口 claim 承兑；
- dispute 和 slashing 记录；
- remote checkpoint observation。

## C.3 Causal checkpoint mesh

不同恒星系不形成一条全局链，而形成一个 checkpoint DAG：

```text
Earth C100 ── Earth C101 ── Earth C102
    │                         │
    │ observes Alpha C70      │ observes Barnard C55
    ▼                         ▼
Alpha C70 ── Alpha C71       Barnard C55 ── Barnard C56
```

边有两类：

1. **本地链边**
   - `prev_checkpoint_hash`
   - 同一 domain 内 total order。
2. **跨域 observation 边**
   - 某 checkpoint 记录了对远方 checkpoint 的观察。
   - 表示“该 domain 在该时刻已经知道远方某历史”。

这个 DAG 只表达因果关系，不产生全局 total order。

## C.4 Asset model：mixed model

采用混合模型：

```text
本地账户 balance:
  适合本地普通支付

Object-capability asset:
  适合稀缺资产、抵押品、NFT、船舶产权、矿权

UTXO-like export lock:
  适合跨星系 claim

Contract state object:
  适合远期合同、保险池、信用额度
```

每个跨星系资产必须有一个 home domain：

```text
AssetId = Hash(home_domain_id, asset_class, local_nonce)
```

资产的真实稀缺性由 home domain ledger 维护。
远方得到的不是原资产本身，而是：

```text
claim-backed credit
or
settlement claim
or
redeemable remote representation
```

## C.5 跨星系 checkpoint 交换

每个 domain 周期性生成 checkpoint：

```text
every checkpoint_interval:
    finalize local block range
    compute roots
    include observed remote checkpoint hashes
    threshold-sign checkpoint
    broadcast locally
    export to remote relays
```

跨星系传播内容通常是：

```text
CheckpointBundle = {
  checkpoint,
  validator_set_proof,
  epoch_transition_proof,
  selected Merkle proofs,
  causal_certificate,
  relay_observations
}
```

远方客户端验证：

1. checkpoint threshold signature；
2. validator set 是否从可信 genesis 或可信 anchor 合法演进；
3. Merkle proof 是否证明交易在 checkpoint 中；
4. checkpoint 是否与已知历史冲突；
5. causal certificate 是否满足光锥约束；
6. 该 checkpoint 是否足够新，足够支持当前 risk policy。

## C.6 Finality 层级

| 状态                        | 含义                                | 可升级条件                   |
| --------------------------- | ----------------------------------- | ---------------------------- |
| `pending-local`             | 本地交易已提交但未 final            | 本地 BFT commit              |
| `locally-final`             | 本地 ledger 已 final                | checkpoint 包含该交易        |
| `exported-to-remote`        | 生成了跨域 claim 并开始传播         | 远方 observation             |
| `remote-observed`           | 远方验证了 origin checkpoint        | 远方 ledger 记录 observation |
| `provisionally-credited`    | 远方基于信用额度临时入账            | 风险策略允许                 |
| `accepted-by-remote-ledger` | 远方 ledger 正式记录承兑            | receipt 回传                 |
| `origin-acknowledged`       | origin ledger 收到远方承兑回执      | origin checkpoint 包含 ack   |
| `bilaterally-settled`       | 双方都已观察对方确认                | 双向 checkpoint 闭环完成     |
| `disputed`                  | 出现冲突、过期、无效证明或治理争议  | dispute resolution           |
| `expired`                   | claim 超过有效期或 freshness window | 重新提交或作废               |
| `slashed`                   | 作恶证据触发罚没                    | slashing finality            |

## C.7 强保证与风险管理边界

| 机制                | 强保证                            | 不能保证                              |
| ------------------- | --------------------------------- | ------------------------------------- |
| Merkle proof        | 某交易被某 checkpoint 承诺        | checkpoint 内容真实合法之外的经济偿付 |
| Threshold signature | 某 validator quorum 签过          | quorum 未串谋                         |
| Local BFT           | 本地 honest quorum 下不可回滚     | 本地治理不被俘获                      |
| Lightcone proof     | 某知识传播在物理上可能            | 坐标 attestation 永远真实             |
| Credit limit        | 最大账面敞口受限                  | 对方一定赔偿                          |
| Slashing evidence   | 能证明 equivocation / double-sign | 能实际执行跨政权罚没                  |
| Insurance pool      | 可覆盖部分损失                    | 极端系统性违约                        |

------

# D. 客户端状态机

## D.1 客户端角色

| 客户端角色              | 职责                                        |
| ----------------------- | ------------------------------------------- |
| 普通钱包 `WalletClient` | 管理密钥、提交本地交易、查看 claim 状态     |
| Light client            | 只保存 checkpoints、Merkle proofs、相关交易 |
| Clearing client         | 管理信用额度、抵押、承兑、清算、折价        |
| Validator client        | 参与本地 BFT 共识                           |
| Archive client          | 保存完整历史、提供证明                      |
| Shipborne relay         | 携带旧 checkpoint、离线数据包、延迟转发     |
| AI financial agent      | 自动报价、承兑、拒绝、对冲、开 dispute      |
| Watcher client          | 监控冲突 checkpoint、双花、过期 claim       |

## D.2 客户端本地存储

普通 light client 至少保存：

```text
ClientLocalStore
  ├── key material
  │   ├── spending keys
  │   ├── viewing keys
  │   ├── recovery policy
  │   └── hardware / MPC metadata
  │
  ├── trusted anchors
  │   ├── genesis checkpoints
  │   ├── validator set roots
  │   ├── known protocol versions
  │   └── crypto suite registry
  │
  ├── checkpoint store
  │   ├── local checkpoints
  │   ├── remote checkpoints
  │   ├── conflicting checkpoints
  │   └── observed checkpoint DAG
  │
  ├── proof store
  │   ├── Merkle proofs
  │   ├── causal certificates
  │   ├── lightcone proofs
  │   └── slashing evidence
  │
  ├── settlement store
  │   ├── outgoing claims
  │   ├── incoming claims
  │   ├── receipts
  │   ├── disputes
  │   └── status history
  │
  ├── risk policy
  │   ├── credit limits
  │   ├── accepted domains
  │   ├── haircut curves
  │   ├── settlement horizons
  │   └── blocked / quarantined domains
  │
  └── sync metadata
      ├── last_seen_height per domain
      ├── causal frontier
      ├── replay cache
      ├── offline intervals
      └── deterministic replay log
```

## D.3 客户端状态机

核心状态：

```text
ClaimStatus =
  Unknown
  PendingLocal
  LocallyFinal
  ExportedToRemote
  RemoteObserved
  ProvisionallyCredited
  AcceptedByRemoteLedger
  OriginAcknowledged
  BilaterallySettled
  Disputed
  Expired
  Slashed
  Rejected
```

状态转移必须是单调的，不能跳过风险层级：

```text
PendingLocal
  -> LocallyFinal
  -> ExportedToRemote
  -> RemoteObserved
  -> ProvisionallyCredited
  -> AcceptedByRemoteLedger
  -> OriginAcknowledged
  -> BilaterallySettled
```

任何阶段都可能进入：

```text
Disputed
Expired
Rejected
Slashed
```

强制规则：

```text
ProvisionallyCredited != BilaterallySettled
RemoteObserved != GlobalFinal
AbsenceOfConflict != ProofOfNoConflict
```

## D.4 同步策略

客户端同步不是“追上全局链”，而是追上若干 domain 的 checkpoint frontier：

```text
SyncTarget = {
  local_domain,
  asset_home_domains,
  counterparty_domains,
  clearing_domains,
  watcher_domains
}
```

同步步骤：

1. 从多个 archive / relay / peer 获取 checkpoint bundles。
2. 验证每个 checkpoint 的 threshold signature。
3. 验证 validator set evolution。
4. 验证本地 hash chain。
5. 插入 causal checkpoint mesh。
6. 检测冲突。
7. 更新相关 claim 状态。
8. 对过期或风险过高的 claim 降级。
9. 输出用户可理解的 settlement status。

## D.5 处理过期 checkpoint

Checkpoint 本身作为历史证据不“过期”。
但它可能不再足够新，不能支撑新的信用承兑。

规则：

```text
if checkpoint.age > freshness_window:
    may_use_as_ancestry_proof = true
    may_use_for_new_credit = false unless policy allows
```

旧 checkpoint 可以证明历史。
旧 checkpoint 不能证明“当前没有冲突”。

## D.6 处理冲突 checkpoint

冲突类型：

```text
SameHeightFork:
  same domain, same height, different checkpoint hash, both valid QC

DoubleSpendFork:
  same asset locked/spent in two incompatible finalized branches

InvalidEpochTransition:
  validator set transition has no valid authorization

CausalImpossibleCheckpoint:
  checkpoint claims to observe remote event before light could arrive
```

处理策略：

```text
on conflict:
    quarantine(domain)
    mark related claims as Disputed
    create DisputeRecord
    freeze provisional credits if policy requires
    broadcast slashing evidence
    require governance / insurance / collateral resolution
```

## D.7 长时间离线后重新加入

离线客户端不能简单信任第一个 peer。

重同步流程：

```text
1. Load last trusted anchors.
2. Request checkpoint bundles from independent peers.
3. Verify all checkpoint chains from trusted anchors.
4. Reject physically impossible observations.
5. Detect equivocation and forks.
6. Rebuild only relevant local state from Merkle proofs.
7. Recompute claim status.
8. Mark stale risk assumptions.
9. Require user / policy confirmation before spending affected assets.
```

## D.8 来自不同光锥历史的信息

客户端维护的是：

```text
KnownFrontier = Map<DomainId, Set<CheckpointHash>>
```

同一个 domain 正常情况下应该只有一个 frontier head。
若出现多个 head：

```text
if heads are compatible:
    keep latest descendant

if heads conflict:
    quarantine domain
    do not merge blindly
```

对 spacelike 事件：

```text
if neither A happens-before B nor B happens-before A:
    they are concurrent
    client must not invent order
```

## D.9 核心 API

```text
submit_local_tx(tx) -> LocalTxReceipt

lock_for_export(
    asset_id,
    amount,
    destination_domain,
    beneficiary,
    expiry,
    risk_policy
) -> ExportLockReceipt

create_settlement_claim(lock_receipt) -> SettlementClaimBundle

verify_remote_checkpoint(bundle) -> VerificationReport

verify_settlement_claim(claim_bundle) -> ClaimVerificationReport

accept_remote_claim(claim_bundle, policy) -> AcceptReceipt | RejectReason

provisionally_credit(claim_id, amount, haircut, credit_line) -> CreditReceipt

get_claim_status(claim_id) -> ClaimStatusReport

open_dispute(evidence_bundle) -> DisputeRecord

resync_after_offline(sync_bundle, policy) -> ResyncReport

quote_risk(claim_bundle, horizon, credit_line) -> RiskQuote
```

------

# E. 数据结构

以下是 Rust-like 伪代码，强调字段语义而非具体语法。

## E.1 基础类型

```rust
type Hash = [u8; 32];
type DomainId = Hash;
type AssetId = Hash;
type ClaimId = Hash;
type TxId = Hash;
type EventId = Hash;
type CheckpointHash = Hash;
type PublicKey = Vec<u8>;
type Signature = Vec<u8>;

struct TimeInterval {
    min: i128,   // reference-frame ticks
    max: i128,
}

struct Region3D {
    center: [f64; 3],
    radius_ly: f64,
}

struct SpacetimeCoord {
    frame_id: Hash,
    time: TimeInterval,
    region: Region3D,
    uncertainty: f64,
    attestations: Vec<Signature>,
}
```

## E.2 LedgerEvent

```rust
enum LedgerEventKind {
    TxSubmitted,
    TxCommitted,
    CheckpointCreated,
    RemoteCheckpointObserved,
    ExportLocked,
    RemoteClaimAccepted,
    SettlementReceiptCreated,
    OriginAcknowledged,
    DisputeOpened,
    SlashingExecuted,
}

struct LedgerEvent {
    event_id: EventId,
    domain_id: DomainId,
    actor: PublicKey,
    kind: LedgerEventKind,

    local_height: u64,
    local_sequence: u64,

    payload_hash: Hash,
    dependencies: Vec<EventId>,

    coord: SpacetimeCoord,

    signatures: Vec<Signature>,
}
```

## E.3 Transaction

```rust
enum TxKind {
    TransferLocal,
    LockForExport,
    AcceptRemoteClaim,
    FinalizeSettlement,
    OpenDispute,
    Slash,
    UpdateCreditLine,
    ContractCall,
}

struct TxInput {
    asset_id: AssetId,
    owner_proof: Vec<u8>,
    amount: u128,
}

struct TxOutput {
    asset_id: AssetId,
    new_owner: PublicKey,
    amount: u128,
    restrictions: Vec<Hash>,
}

struct Transaction {
    tx_id: TxId,
    domain_id: DomainId,
    kind: TxKind,

    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,

    nonce: u128,
    valid_after: Option<i128>,
    valid_until: Option<i128>,

    referenced_claims: Vec<ClaimId>,
    causal_deps: Vec<EventId>,

    fee: u128,
    signer_keys: Vec<PublicKey>,
    signatures: Vec<Signature>,
}
```

## E.4 SettlementClaim

```rust
enum ClaimStatusOnOrigin {
    Locked,
    Redeemed,
    Cancelled,
    Disputed,
    Slashed,
}

struct SettlementClaim {
    claim_id: ClaimId,

    origin_domain: DomainId,
    destination_domain: DomainId,

    beneficiary: PublicKey,
    asset_ids: Vec<AssetId>,
    amount: u128,
    denomination: Hash,

    lock_tx_id: TxId,
    lock_event_id: EventId,

    origin_checkpoint_hash: CheckpointHash,
    origin_checkpoint_height: u64,

    merkle_proof_lock_included: Vec<Hash>,
    merkle_proof_asset_locked: Vec<Hash>,

    earliest_send_coord: SpacetimeCoord,
    expiry_origin_time: i128,

    max_haircut_bps: u32,
    min_collateral_ratio_bps: u32,

    causal_certificate: CausalCertificate,
    lightcone_proof: LightconeProof,

    origin_quorum_certificate: QuorumCertificate,
}
```

## E.5 Checkpoint

```rust
struct Checkpoint {
    domain_id: DomainId,
    height: u64,
    epoch: u64,

    prev_checkpoint_hash: Option<CheckpointHash>,

    state_root: Hash,
    tx_root: Hash,
    event_log_root: Hash,

    asset_root: Hash,
    export_root: Hash,
    import_root: Hash,
    credit_root: Hash,
    dispute_root: Hash,

    observed_remote_root: Hash,

    validator_set_root: Hash,
    protocol_version: u32,
    crypto_suite_id: Hash,

    coord: SpacetimeCoord,

    quorum_certificate: QuorumCertificate,
    post_quantum_certificate: Option<QuorumCertificate>,
}

struct QuorumCertificate {
    domain_id: DomainId,
    epoch: u64,
    message_hash: Hash,
    threshold: u32,
    signer_bitmap: Vec<u8>,
    aggregate_signature: Signature,
}
```

## E.6 CausalCertificate

```rust
struct CausalDependency {
    event_id: EventId,
    event_hash: Hash,
    checkpoint_hash: Option<CheckpointHash>,
    merkle_path: Vec<Hash>,
}

struct CausalFrontierEntry {
    domain_id: DomainId,
    checkpoint_height: u64,
    checkpoint_hash: CheckpointHash,
}

struct CausalCertificate {
    certificate_id: Hash,

    subject_hash: Hash,

    dependencies: Vec<CausalDependency>,

    frontier: Vec<CausalFrontierEntry>,

    observations: Vec<Observation>,

    lightcone_proofs: Vec<LightconeProof>,

    issuer: PublicKey,
    signature: Signature,
}
```

## E.7 LightconeProof

```rust
struct Observation {
    observer_id: PublicKey,
    observer_domain: DomainId,

    observed_hash: Hash,
    observed_kind: String,

    receive_event_id: EventId,
    receive_coord: SpacetimeCoord,

    source_hint: Option<Hash>,
    signature: Signature,
}

struct LightconeHop {
    from_event: EventId,
    to_event: EventId,

    send_coord: SpacetimeCoord,
    receive_coord: SpacetimeCoord,

    channel_kind: String, // laser, radio, ship, relay, local
    relay_id: Option<PublicKey>,

    payload_hash: Hash,

    signatures: Vec<Signature>,
}

struct LightconeProof {
    proof_id: Hash,

    origin_event: EventId,
    terminal_event: EventId,

    hops: Vec<LightconeHop>,

    max_uncertainty: f64,

    route_commitment: Hash,

    signatures: Vec<Signature>,
}
```

## E.8 CreditLine

```rust
enum CreditLineStatus {
    Active,
    Suspended,
    Exhausted,
    InDispute,
    Expired,
}

struct HaircutPoint {
    days_to_settlement: u64,
    haircut_bps: u32,
}

struct CreditLine {
    credit_line_id: Hash,

    creditor_domain: DomainId,
    debtor_domain: DomainId,

    limit: u128,
    outstanding: u128,

    collateral_asset_ids: Vec<AssetId>,
    collateral_domain: DomainId,
    collateral_ratio_bps: u32,

    insurance_pool_id: Option<Hash>,

    haircut_curve: Vec<HaircutPoint>,

    valid_from: i128,
    valid_until: i128,

    status: CreditLineStatus,

    authorized_signers: Vec<PublicKey>,
    signatures: Vec<Signature>,
}
```

## E.9 DisputeRecord

```rust
enum DisputeKind {
    ConflictingCheckpoint,
    DoubleSpend,
    InvalidProof,
    LightconeViolation,
    ExpiredClaimReplay,
    ValidatorEquivocation,
    GovernanceDefault,
}

enum DisputeStatus {
    Open,
    EvidenceConfirmed,
    SlashingPending,
    Slashed,
    Resolved,
    Unresolvable,
}

struct DisputeRecord {
    dispute_id: Hash,

    kind: DisputeKind,

    accused_domain: DomainId,
    affected_claims: Vec<ClaimId>,
    affected_assets: Vec<AssetId>,

    evidence_hashes: Vec<Hash>,
    evidence_bundle_root: Hash,

    opened_by: PublicKey,
    opened_at_event: EventId,

    status: DisputeStatus,

    required_actions: Vec<String>,

    signatures: Vec<Signature>,
}
```

## E.10 ClientSyncState

```rust
struct ClientSyncState {
    client_id: PublicKey,

    home_domain: DomainId,

    trusted_anchors: Vec<CheckpointHash>,

    known_checkpoints: Map<DomainId, Vec<CheckpointHash>>,

    frontier: Map<DomainId, Vec<CheckpointHash>>,

    quarantined_domains: Set<DomainId>,

    pending_claims: Map<ClaimId, SettlementClaim>,

    claim_status: Map<ClaimId, ClaimStatus>,

    credit_lines: Map<Hash, CreditLine>,

    replay_cache: Set<Hash>,

    last_sync_coord: Option<SpacetimeCoord>,

    offline_since: Option<i128>,

    risk_policy_hash: Hash,
}
```

## E.11 网络消息格式

```rust
enum NetworkMessage {
    CheckpointBundle {
        checkpoint: Checkpoint,
        validator_proof: Vec<u8>,
        ancestry_proof: Vec<Checkpoint>,
        causal_certificate: Option<CausalCertificate>,
    },

    SettlementClaimBundle {
        claim: SettlementClaim,
        checkpoint_bundle: Box<NetworkMessage>,
        additional_proofs: Vec<Vec<u8>>,
    },

    SettlementReceiptBundle {
        claim_id: ClaimId,
        accepting_domain: DomainId,
        accept_tx_id: TxId,
        accept_checkpoint: Checkpoint,
        merkle_proof: Vec<Hash>,
        causal_certificate: CausalCertificate,
    },

    DisputeEvidenceBundle {
        dispute: DisputeRecord,
        evidence_items: Vec<Vec<u8>>,
    },

    SyncRequest {
        requester: PublicKey,
        wanted_domains: Vec<DomainId>,
        from_heights: Map<DomainId, u64>,
        max_bytes: u64,
    },

    SyncResponse {
        bundles: Vec<NetworkMessage>,
    },
}
```

------

# F. 协议流程

## F.1 本地交易提交：`submit_local_tx`

```python
def submit_local_tx(client, tx):
    assert tx.domain_id == client.home_domain
    assert verify_signatures(tx)
    assert tx.nonce not in client.replay_cache
    assert tx.valid_until is None or now_local() <= tx.valid_until

    send_to_local_mempool(tx)

    receipt = wait_for_local_bft_commit(tx.tx_id)

    if not receipt.committed:
        return Reject("not committed")

    checkpoint = wait_for_checkpoint_including(tx.tx_id)

    assert verify_checkpoint(checkpoint)
    assert verify_merkle_inclusion(tx.tx_id, checkpoint.tx_root, receipt.merkle_proof)

    client.replay_cache.add(tx.nonce)
    client.known_checkpoints[tx.domain_id].append(checkpoint.hash)

    return LocalTxReceipt(
        tx_id=tx.tx_id,
        status="locally-final",
        checkpoint_hash=checkpoint.hash,
        checkpoint_height=checkpoint.height,
        merkle_proof=receipt.merkle_proof
    )
```

## F.2 出口锁定：`lock_for_export`

```python
def lock_for_export(
    client,
    asset_id,
    amount,
    destination_domain,
    beneficiary,
    expiry,
    risk_policy
):
    asset = client.lookup_asset(asset_id)

    if asset.home_domain != client.home_domain:
        return Reject("asset not native to this domain")

    if asset.status not in ["Owned", "Spendable"]:
        return Reject("asset not spendable")

    tx = Transaction(
        domain_id=client.home_domain,
        kind="LockForExport",
        inputs=[TxInput(asset_id=asset_id, amount=amount)],
        outputs=[],
        nonce=fresh_nonce(),
        valid_until=expiry,
        signer_keys=[client.spend_key],
        causal_deps=[],
    )

    tx.payload = {
        "destination_domain": destination_domain,
        "beneficiary": beneficiary,
        "amount": amount,
        "expiry": expiry,
        "risk_policy": risk_policy.hash,
    }

    sign(tx, client.spend_key)

    receipt = submit_local_tx(client, tx)

    if receipt.status != "locally-final":
        return Reject("lock failed")

    return ExportLockReceipt(
        asset_id=asset_id,
        amount=amount,
        destination_domain=destination_domain,
        beneficiary=beneficiary,
        lock_tx_id=tx.tx_id,
        checkpoint_hash=receipt.checkpoint_hash,
        merkle_proof=receipt.merkle_proof,
        status="locally-final"
    )
```

## F.3 创建 settlement claim：`create_settlement_claim`

```python
def create_settlement_claim(client, lock_receipt):
    checkpoint = client.get_checkpoint(lock_receipt.checkpoint_hash)

    assert verify_checkpoint(checkpoint)
    assert verify_merkle_inclusion(
        lock_receipt.lock_tx_id,
        checkpoint.tx_root,
        lock_receipt.merkle_proof
    )

    lock_event = client.get_event_for_tx(lock_receipt.lock_tx_id)

    causal_cert = build_causal_certificate(
        subject_hash=lock_receipt.lock_tx_id,
        dependencies=[lock_event.event_id, checkpoint.hash],
        frontier=client.current_frontier(),
        observations=[]
    )

    lightcone_proof = build_initial_lightcone_proof(
        origin_event=lock_event.event_id,
        terminal_event=lock_event.event_id,
        local_coord=lock_event.coord
    )

    claim_id = hash_many([
        client.home_domain,
        lock_receipt.lock_tx_id,
        lock_receipt.asset_id,
        lock_receipt.destination_domain,
        lock_receipt.beneficiary
    ])

    claim = SettlementClaim(
        claim_id=claim_id,
        origin_domain=client.home_domain,
        destination_domain=lock_receipt.destination_domain,
        beneficiary=lock_receipt.beneficiary,
        asset_ids=[lock_receipt.asset_id],
        amount=lock_receipt.amount,
        denomination=get_asset_denomination(lock_receipt.asset_id),
        lock_tx_id=lock_receipt.lock_tx_id,
        lock_event_id=lock_event.event_id,
        origin_checkpoint_hash=checkpoint.hash,
        origin_checkpoint_height=checkpoint.height,
        merkle_proof_lock_included=lock_receipt.merkle_proof,
        merkle_proof_asset_locked=get_asset_state_proof(lock_receipt.asset_id, checkpoint),
        earliest_send_coord=lock_event.coord,
        expiry_origin_time=lock_receipt.expiry,
        max_haircut_bps=client.policy.max_haircut_bps,
        min_collateral_ratio_bps=client.policy.min_collateral_ratio_bps,
        causal_certificate=causal_cert,
        lightcone_proof=lightcone_proof,
        origin_quorum_certificate=checkpoint.quorum_certificate
    )

    client.pending_claims[claim_id] = claim
    client.claim_status[claim_id] = "exported-to-remote"

    return SettlementClaimBundle(claim=claim)
```

## F.4 远方 checkpoint 验证：`verify_remote_checkpoint`

```python
def verify_remote_checkpoint(client, bundle):
    cp = bundle.checkpoint

    if cp.domain_id in client.quarantined_domains:
        return Reject("domain quarantined")

    if not verify_protocol_version(cp.protocol_version):
        return Reject("unsupported protocol version")

    if not verify_validator_set_proof(
        domain_id=cp.domain_id,
        epoch=cp.epoch,
        validator_set_root=cp.validator_set_root,
        proof=bundle.validator_proof,
        trusted_anchors=client.trusted_anchors
    ):
        return Reject("invalid validator set proof")

    if not verify_quorum_certificate(cp.quorum_certificate, cp.validator_set_root):
        return Reject("invalid quorum certificate")

    if cp.prev_checkpoint_hash is not None:
        if not verify_ancestry(cp, bundle.ancestry_proof, client.known_checkpoints):
            return Reject("invalid checkpoint ancestry")

    if bundle.causal_certificate is not None:
        if not verify_causal_certificate(client, bundle.causal_certificate):
            return Reject("invalid causal certificate")

    if contains_lightcone_violation(cp, bundle):
        return Reject("lightcone violation")

    conflict = detect_conflict(client, cp)

    if conflict is not None:
        record = slash_or_dispute(client, conflict)
        return RejectWithDispute(record)

    store_checkpoint(client, cp)

    return Accept(
        checkpoint_hash=hash(cp),
        domain_id=cp.domain_id,
        height=cp.height
    )
```

## F.5 claim 验证：`verify_settlement_claim`

```python
def verify_settlement_claim(client, claim_bundle):
    claim = claim_bundle.claim

    if claim.destination_domain != client.home_domain:
        return Reject("claim not addressed to this domain")

    if claim.claim_id in client.replay_cache:
        return Reject("replay detected")

    cp_report = verify_remote_checkpoint(client, claim_bundle.checkpoint_bundle)

    if not cp_report.accepted:
        return Reject("origin checkpoint invalid")

    origin_cp = claim_bundle.checkpoint_bundle.checkpoint

    if hash(origin_cp) != claim.origin_checkpoint_hash:
        return Reject("checkpoint hash mismatch")

    if not verify_merkle_inclusion(
        claim.lock_tx_id,
        origin_cp.tx_root,
        claim.merkle_proof_lock_included
    ):
        return Reject("lock tx not included")

    if not verify_asset_locked_state(
        claim.asset_ids,
        origin_cp.asset_root,
        claim.merkle_proof_asset_locked
    ):
        return Reject("asset not locked")

    if not verify_causal_certificate(client, claim.causal_certificate):
        return Reject("invalid causal certificate")

    if not verify_lightcone_proof(client, claim.lightcone_proof):
        return Reject("invalid lightcone proof")

    if claim.expiry_origin_time < origin_cp.coord.time.min:
        return Reject("claim expired at origin")

    if has_local_conflicting_claim(client, claim):
        return Reject("conflicting claim already known locally")

    return Accept("claim valid but not necessarily risk-free")
```

## F.6 远方承兑：`accept_remote_claim`

```python
def accept_remote_claim(client, claim_bundle, policy):
    report = verify_settlement_claim(client, claim_bundle)

    if not report.accepted:
        return Reject(report.reason)

    claim = claim_bundle.claim

    risk = evaluate_claim_risk(
        claim=claim,
        known_frontier=client.frontier,
        credit_lines=client.credit_lines,
        settlement_horizon=estimate_settlement_horizon(client, claim),
        domain_reputation=policy.domain_reputation
    )

    if risk.score > policy.max_risk_score:
        return Reject("risk too high")

    credit_line = select_credit_line(
        creditor_domain=client.home_domain,
        debtor_domain=claim.origin_domain,
        amount=claim.amount
    )

    if credit_line is None:
        return Reject("no credit line")

    if credit_line.outstanding + claim.amount > credit_line.limit:
        return Reject("credit limit exceeded")

    tx = Transaction(
        domain_id=client.home_domain,
        kind="AcceptRemoteClaim",
        inputs=[],
        outputs=[
            TxOutput(
                asset_id=make_claim_backed_asset_id(claim.claim_id),
                new_owner=claim.beneficiary,
                amount=discount(claim.amount, risk.haircut_bps),
                restrictions=[hash("claim-backed"), claim.claim_id]
            )
        ],
        nonce=fresh_nonce(),
        referenced_claims=[claim.claim_id],
        causal_deps=[claim.lock_event_id],
        signer_keys=[client.clearing_key],
    )

    tx.payload = {
        "claim_id": claim.claim_id,
        "origin_domain": claim.origin_domain,
        "credit_line_id": credit_line.credit_line_id,
        "risk_score": risk.score,
        "haircut_bps": risk.haircut_bps,
        "settlement_horizon": risk.settlement_horizon,
        "status": "provisionally-credited"
    }

    sign(tx, client.clearing_key)

    receipt = submit_local_tx(client, tx)

    if receipt.status != "locally-final":
        return Reject("local accept tx failed")

    credit_line.outstanding += claim.amount

    client.pending_claims[claim.claim_id] = claim
    client.claim_status[claim.claim_id] = "provisionally-credited"
    client.replay_cache.add(claim.claim_id)

    return AcceptReceipt(
        claim_id=claim.claim_id,
        accept_tx_id=tx.tx_id,
        accepting_domain=client.home_domain,
        checkpoint_hash=receipt.checkpoint_hash,
        status="provisionally-credited",
        risk=risk
    )
```

## F.7 临时入账：`provisionally_credit`

```python
def provisionally_credit(client, claim_id, amount, credit_line_id, haircut_bps):
    claim = client.pending_claims[claim_id]
    credit_line = client.credit_lines[credit_line_id]

    assert credit_line.status == "Active"
    assert credit_line.outstanding + amount <= credit_line.limit
    assert haircut_bps >= required_haircut(client.policy, claim)

    provisional_amount = amount * (10_000 - haircut_bps) // 10_000

    ledger_object = {
        "kind": "ClaimBackedCredit",
        "claim_id": claim_id,
        "origin_domain": claim.origin_domain,
        "amount": provisional_amount,
        "not_final_before": estimate_settlement_horizon(client, claim),
        "risk_label": compute_risk_label(client, claim),
    }

    tx = make_mint_claim_backed_credit_tx(ledger_object)
    receipt = submit_local_tx(client, tx)

    if receipt.status == "locally-final":
        client.claim_status[claim_id] = "provisionally-credited"
        return CreditReceipt(ledger_object, receipt)

    return Reject("credit mint failed")
```

## F.8 checkpoint 合并：`reconcile_checkpoint`

```python
def reconcile_checkpoint(client, checkpoint_bundle):
    report = verify_remote_checkpoint(client, checkpoint_bundle)

    if not report.accepted:
        return report

    cp = checkpoint_bundle.checkpoint
    add_to_checkpoint_mesh(client, cp)

    observed = extract_observed_remote_checkpoints(cp)

    for remote_cp_hash in observed:
        add_causal_edge(
            from_checkpoint=remote_cp_hash,
            to_checkpoint=hash(cp),
            kind="observed-by"
        )

    related_claims = find_claims_affected_by_checkpoint(client, cp)

    for claim_id in related_claims:
        old_status = client.claim_status[claim_id]
        new_status = recompute_claim_status(client, claim_id)

        if violates_status_lattice(old_status, new_status):
            raise CriticalError("illegal status promotion")

        client.claim_status[claim_id] = new_status

    return Accept("checkpoint reconciled")
```

## F.9 冲突检测：`detect_conflict`

```python
def detect_conflict(client, new_cp):
    known = client.known_checkpoints.get(new_cp.domain_id, [])

    for old_hash in known:
        old_cp = load_checkpoint(old_hash)

        if old_cp.height == new_cp.height and hash(old_cp) != hash(new_cp):
            if verify_quorum_certificate(old_cp.quorum_certificate, old_cp.validator_set_root) \
               and verify_quorum_certificate(new_cp.quorum_certificate, new_cp.validator_set_root):
                return Conflict(
                    kind="ConflictingCheckpoint",
                    checkpoints=[old_cp, new_cp]
                )

        if same_epoch_invalid_transition(old_cp, new_cp):
            return Conflict(
                kind="InvalidEpochTransition",
                checkpoints=[old_cp, new_cp]
            )

    double_spend = detect_asset_double_spend(client, new_cp)

    if double_spend is not None:
        return Conflict(
            kind="DoubleSpend",
            evidence=double_spend
        )

    if checkpoint_contains_impossible_observation(new_cp):
        return Conflict(
            kind="LightconeViolation",
            evidence=new_cp
        )

    return None
```

## F.10 dispute / slashing：`slash_or_dispute`

```python
def slash_or_dispute(client, conflict):
    evidence = build_evidence_bundle(conflict)

    if conflict.kind in [
        "ConflictingCheckpoint",
        "ValidatorEquivocation",
        "DoubleSpend"
    ]:
        slashable = verify_slashing_conditions(evidence)
    else:
        slashable = False

    dispute = DisputeRecord(
        dispute_id=hash(evidence),
        kind=conflict.kind,
        accused_domain=conflict.accused_domain,
        affected_claims=find_affected_claims(client, conflict),
        affected_assets=find_affected_assets(client, conflict),
        evidence_hashes=[hash(x) for x in evidence.items],
        evidence_bundle_root=merkle_root(evidence.items),
        opened_by=client.client_id,
        opened_at_event=current_local_event_id(),
        status="EvidenceConfirmed" if slashable else "Open",
        required_actions=[],
        signatures=[sign(hash(evidence), client.dispute_key)]
    )

    submit_local_tx(client, make_open_dispute_tx(dispute))

    for claim_id in dispute.affected_claims:
        client.claim_status[claim_id] = "disputed"

    if slashable:
        submit_local_tx(client, make_slashing_tx(dispute))
        dispute.status = "SlashingPending"

    broadcast_dispute_evidence(dispute, evidence)

    return dispute
```

## F.11 离线重同步：`resync_after_offline`

```python
def resync_after_offline(client, sync_bundles, policy):
    candidate_checkpoints = []

    for bundle in sync_bundles:
        report = verify_remote_checkpoint(client, bundle)

        if report.accepted:
            candidate_checkpoints.append(bundle.checkpoint)
        elif report.dispute is not None:
            store_dispute(client, report.dispute)

    groups = group_by_domain(candidate_checkpoints)

    for domain, cps in groups.items():
        heads = compute_compatible_heads(cps)

        if len(heads) == 1:
            client.frontier[domain] = [hash(heads[0])]
        else:
            client.frontier[domain] = [hash(h) for h in heads]
            client.quarantined_domains.add(domain)
            mark_domain_related_claims_disputed(client, domain)

    for claim_id in client.pending_claims:
        status = recompute_claim_status(client, claim_id)
        client.claim_status[claim_id] = status

    stale = find_stale_risk_assumptions(client, policy)

    return ResyncReport(
        accepted_checkpoints=len(candidate_checkpoints),
        quarantined_domains=list(client.quarantined_domains),
        stale_claims=stale,
        current_frontier=client.frontier
    )
```

## F.12 跨星系付款完整流程

以 A 星系向 B 星系付款为例：

```text
1. Alice 在 A ledger 中提交 LockForExport。
2. A ledger 本地 BFT finalizes lock。
3. A 生成 checkpoint CA。
4. Alice / clearing node 生成 SettlementClaim。
5. Claim 通过 relay / laser / shipborne channel 传播。
6. B 多年后收到 claim。
7. B 验证：
   - A checkpoint QC
   - validator set proof
   - Merkle inclusion
   - asset locked state
   - causal certificate
   - lightcone validity
   - no local known conflict
8. B 根据 credit line、haircut、risk policy 决定：
   - reject
   - accept without credit
   - provisionally credit
   - accept at discount
9. B ledger 记录 AcceptRemoteClaim。
10. B 生成 receipt。
11. Receipt 返回 A。
12. A ledger 记录 OriginAcknowledged。
13. A 的 acknowledgement checkpoint 返回 B。
14. B 将状态升级为 BilaterallySettled。
```

关键原则：

> B 在第 8 步不能说“最终到账”。
> 只能说“基于 A 的 signed lock proof 和信用额度，临时承兑”。

------

# G. 威胁模型与缓解

| 威胁                           | 失败模式                                    | 缓解                                                         | 保证类型                 |
| ------------------------------ | ------------------------------------------- | ------------------------------------------------------------ | ------------------------ |
| 恶意客户端伪造时间戳           | 声称提前收到远方消息                        | 使用 SpacetimeCoord、observatory attestation、LightconeProof；验证每一跳是否可能 | 强保证依赖坐标证明       |
| 清算节点签发冲突 checkpoint    | 同一高度两个状态                            | threshold QC conflict 作为 slashing evidence；客户端 quarantine domain | 密码学检测强，赔偿靠治理 |
| 多个远方分区同时接受同一资产   | Alpha 和 Barnard 都临时承兑同一 asset claim | home ledger honest 时本地防止；若 origin equivocation，则事后 dispute/slash；credit limit 限制损失 | 风险管理                 |
| relay 延迟消息                 | 用户误以为无冲突                            | 不把 absence of evidence 当 final；freshness window；多 relay；watcher network | 风险管理                 |
| relay 截断 / 选择性转发        | eclipse 或隐瞒 dispute                      | 多路径同步；independent archives；shipborne delayed bundles；gossip diversity | 风险管理                 |
| 长距离 eclipse attack          | 客户端只看到攻击者构造历史                  | trusted anchors；多源 checkpoint；quarantine unexpected forks；需要独立 witness | 风险管理 + 部分强保证    |
| 历史重写                       | 伪造旧链或替换历史                          | hash chain；threshold checkpoint；archive cross-witness；client pinning trusted anchors | 本地强保证               |
| 本地政权 / 委员会失信          | 合法 quorum 作恶                            | slashing、保险、信用降级、domain quarantine、治理退出        | 不能纯技术解决           |
| 飞船携带旧数据 replay          | 20 年前 claim 被重新提交                    | claim_id replay cache；expiry；nonce；checkpoint freshness policy | 强保证                   |
| 旧 checkpoint 被重新用于新信用 | 过期历史冒充新状态                          | checkpoint 可作历史证明，但不能单独支撑新承兑                | 强保证 + policy          |
| AI 代理被攻破                  | 自动承兑高风险 claim                        | policy sandbox；spending caps；MPC approval；rate limit；explainable risk log | 风险管理                 |
| 私钥丢失                       | 资产永久不可用                              | social recovery；threshold wallet；timelock recovery；dead-man switch | 风险管理                 |
| 私钥被盗                       | 攻击者提交合法交易                          | hardware key；MPC；policy engine；withdrawal delay；watcher cancel window | 风险管理                 |
| 长期密码学算法被破解           | 古老签名失效                                | protocol eras；hash/signature agility；PQ migration；checkpoint re-signing | 风险管理                 |
| 量子攻击                       | ECDSA/EdDSA 失效                            | hybrid classical + PQ signatures；长期资产强制迁移           | 风险管理                 |
| 本地 BFT 少数作恶              | 审查、延迟、DoS                             | BFT liveness under partial synchrony；fee markets；validator rotation | 条件强保证               |
| 本地 BFT 多数作恶              | 双签、没收、审查                            | 只能检测和治理；跨域降低信用额度                             | 非技术风险               |

------

# H. 光锥自动化测试系统

测试系统名称：

```text
ChronoLedger Testbench
```

核心要求：

> 测试框架本身必须内建光速约束。
> 任何消息调度器不得产生物理上不可能的正常消息，除非测试明确注入该攻击。

## H.1 模块结构

```text
ChronoLedger Testbench
  ├── SpacetimeSimulator
  ├── StarSystemTopologyGenerator
  ├── NodeWorldlineGenerator
  ├── LightSpeedMessageScheduler
  ├── AdversarialNetworkScheduler
  ├── ByzantineBehaviorInjector
  ├── ClockSkewSimulator
  ├── RelayDelayDropReplaySimulator
  ├── CheckpointConflictGenerator
  ├── DoubleSpendScenarioGenerator
  ├── LongOfflineClientResyncTests
  ├── CrossSystemSettlementLifecycleTests
  ├── FuzzingEngine
  ├── PropertyBasedTestingEngine
  ├── DeterministicReplayEngine
  ├── TraceVisualizer
  └── ScenarioDSL
```

## H.2 Spacetime simulator

```python
class SpacetimeSimulator:
    def __init__(self, c=1.0):
        self.c = c
        self.events = []
        self.messages = []

    def min_arrival_time(self, send_coord, recv_worldline):
        # find earliest t on recv_worldline satisfying lightcone condition
        pass

    def is_lightcone_possible(self, from_event, to_event):
        dt = to_event.coord.time.min - from_event.coord.time.max
        dx = min_distance(from_event.coord.region, to_event.coord.region)
        return dt + EPS >= dx / self.c
```

## H.3 Light-speed message scheduler

普通 scheduler 规则：

```python
arrival_time >= send_time + distance(sender, receiver) / c
```

adversarial scheduler 可以：

- 延迟；
- 丢弃；
- 重放；
- 乱序；
- 选择性转发；
- eclipse 某客户端；
- 插入伪造 timestamp；
- 注入 impossible message。

但 impossible message 必须打标签：

```text
attack_kind = "FTL_INJECTION"
```

oracle 应验证客户端拒绝它。

## H.4 测试类别

| 测试类别                          | 目的                                    | 输入生成                                    | Oracle / expected property                           | Counterexample 最小化          | Trace 记录                            |
| --------------------------------- | --------------------------------------- | ------------------------------------------- | ---------------------------------------------------- | ------------------------------ | ------------------------------------- |
| Lightcone validity                | 客户端拒绝物理不可能消息                | 随机生成坐标、worldline、消息路径           | 若消息不在未来光锥内，则不得进入 accepted store      | 缩短路径、减少节点、收缩时间差 | event coords、hop list、reject reason |
| Local BFT finality                | 本地 ledger 在 honest quorum 下不回滚   | 生成 validator、fault set、交易序列         | 不存在两个 finalized blocks at same height           | 减少 validator 数、交易数      | votes、QC、block tree                 |
| Checkpoint verification           | 验证 QC、Merkle proof、epoch transition | 随机 checkpoint DAG                         | 无效签名或无效 ancestry 必须拒绝                     | 最小 checkpoint chain          | checkpoint hash、validator proof      |
| Checkpoint conflict               | 发现同域同高双 checkpoint               | 注入 equivocation                           | 必须进入 dispute/quarantine                          | 保留两个 checkpoint 和最小 QC  | conflicting signatures                |
| Double spend                      | 同一 asset 两个互斥 claim               | 生成 honest 和 Byzantine origin 两类        | honest origin 不产生双花；Byzantine origin 被检测    | 单 asset、两 destination       | asset state proofs                    |
| Relay delay/drop/replay           | 测试旧数据和乱序                        | scheduler 延迟/丢弃/重放                    | replay 不改变 final 状态；旧 checkpoint 不支撑新信用 | 单 relay、单 claim             | relay log、arrival events             |
| Long offline resync               | 客户端多年离线后重入                    | 随机 offline interval、fork、missing proofs | 不能盲信单源；冲突 domain quarantine                 | 最少 checkpoint bundle         | sync decisions、frontier              |
| Cross-system settlement lifecycle | A lock → B accept → A ack → B settle    | 生成距离、信用额度、延迟                    | 状态按 lattice 单调升级                              | 两 domain、一 claim            | status timeline                       |
| Provisional/final separation      | 防止 UI 错标                            | 随机缺失回执或 checkpoint                   | `provisionally-credited` 不得显示为 final            | 单 claim                       | UI status log                         |
| Credit exposure bound             | 信用额度限制损失                        | 随机 claims、haircuts、limits               | outstanding <= limit                                 | 最少 claim 数                  | credit ledger                         |
| Byzantine timestamp               | 节点伪造过早 observation                | skew/fake coord                             | 必须 reject 或 quarantine                            | 单 observation                 | coord attestations                    |
| Ship old data reconnect           | 飞船 20 年后带旧包                      | moving worldline + old bundles              | 旧数据可作历史，不可 replay spend                    | 单 ship、单 old claim          | ship worldline                        |
| Fuzzing parser/proof              | 防止崩溃和解析绕过                      | mutational fuzzing                          | no panic；invalid proof rejected                     | byte-level shrink              | corpus input                          |
| Deterministic replay              | 复现所有失败                            | 固定 seed、事件 log                         | 同一 seed 同一结果                                   | 二分事件 trace                 | full deterministic log                |

## H.5 失败时记录的 trace

每个失败用例必须输出：

```text
TraceRecord
  ├── seed
  ├── scenario DSL source
  ├── generated topology
  ├── node worldlines
  ├── event list
  ├── message list
  ├── checkpoint DAG
  ├── causal DAG
  ├── client state transitions
  ├── risk decisions
  ├── accepted / rejected proofs
  ├── invariant violated
  └── minimal reproduction bundle
```

Trace visualizer 应支持：

- spacetime diagram；
- checkpoint DAG；
- causal dependency graph；
- claim status timeline；
- credit exposure over time；
- dispute tree；
- lightcone violation highlight。

------

# I. 测试 DSL 示例

## I.1 DSL 草案

```text
scenario <name> {
  frame <frame_id>

  system <name> at (<x>, <y>, <z>) ly
  domain <domain_name> in <system_name> validators <n> tolerate <f>

  node <node_name> kind <wallet|validator|relay|archive|clearing|ship> in <system>
  ship <ship_name> worldline [
      at t=<time> pos=(x,y,z),
      at t=<time> pos=(x,y,z)
  ]

  asset <asset_name> home <domain> owner <node> amount <amount>

  at <domain>.t=<time>:
      <action>

  message <name> from <node> to <node> payload <payload>
      delay <duration>
      mode <laser|radio|ship|adversarial>

  adversary:
      <fault>

  expect:
      <property>
}
```

## I.2 两个恒星系相距 4.3 光年

```text
scenario earth_alpha_basic_distance {
  frame solar_barycentric

  system Earth at (0.0, 0.0, 0.0) ly
  system AlphaCentauri at (4.3, 0.0, 0.0) ly

  domain EarthLedger in Earth validators 7 tolerate 2
  domain AlphaLedger in AlphaCentauri validators 7 tolerate 2

  node Alice kind wallet in Earth
  node Bob kind wallet in AlphaCentauri
  node AlphaClearing kind clearing in AlphaCentauri

  asset EUSD100 home EarthLedger owner Alice amount 100

  at EarthLedger.t=10.0:
      Alice lock_for_export EUSD100 to AlphaLedger beneficiary Bob expiry 30.0

  expect:
      earliest_observation(AlphaLedger, lock(EUSD100)) >= 14.3
}
```

## I.3 Alice 同时向 Alpha 和 Barnard 花同一笔资产

诚实 Earth ledger 下应只允许一个 lock。

```text
scenario honest_origin_prevents_double_spend {
  frame solar_barycentric

  system Earth at (0, 0, 0) ly
  system Alpha at (4.3, 0, 0) ly
  system Barnard at (0, 5.96, 0) ly

  domain EarthLedger in Earth validators 7 tolerate 2
  domain AlphaLedger in Alpha validators 7 tolerate 2
  domain BarnardLedger in Barnard validators 7 tolerate 2

  node Alice kind wallet in Earth
  node Bob kind wallet in Alpha
  node Carol kind wallet in Barnard

  asset CoinX home EarthLedger owner Alice amount 1

  at EarthLedger.t=100.0:
      Alice lock_for_export CoinX to AlphaLedger beneficiary Bob expiry 200.0

  at EarthLedger.t=100.1:
      Alice lock_for_export CoinX to BarnardLedger beneficiary Carol expiry 200.0

  expect:
      count_final_locks(EarthLedger, CoinX) == 1
      no_double_spend_claim_created(CoinX)
}
```

Byzantine Earth committee 下允许测试冲突证明：

```text
scenario byzantine_origin_double_checkpoint {
  frame solar_barycentric

  system Earth at (0, 0, 0) ly
  system Alpha at (4.3, 0, 0) ly
  system Barnard at (0, 5.96, 0) ly

  domain EarthLedger in Earth validators 7 tolerate 2 byzantine 5
  domain AlphaLedger in Alpha validators 7 tolerate 2
  domain BarnardLedger in Barnard validators 7 tolerate 2

  node Alice kind wallet in Earth
  node Bob kind wallet in Alpha
  node Carol kind wallet in Barnard

  asset CoinX home EarthLedger owner Alice amount 1

  adversary:
      EarthLedger.sign_conflicting_checkpoints height 1000 {
          branch A: lock CoinX to AlphaLedger beneficiary Bob
          branch B: lock CoinX to BarnardLedger beneficiary Carol
      }

  expect:
      when AlphaLedger first_receives branch A:
          status(CoinX claim to Bob) in [remote_observed, provisionally_credited]
          status(CoinX claim to Bob) != bilaterally_settled

      when AlphaLedger receives branch B:
          domain_status(EarthLedger) == quarantined
          claim_status(CoinX) == disputed
}
```

## I.4 Alpha 在尚未可能知道 Barnard 状态时如何处理

```text
scenario alpha_cannot_know_barnard_yet {
  frame solar_barycentric

  system Earth at (0, 0, 0) ly
  system Alpha at (4.3, 0, 0) ly
  system Barnard at (0, 5.96, 0) ly

  domain EarthLedger in Earth validators 7 tolerate 2
  domain AlphaLedger in Alpha validators 7 tolerate 2
  domain BarnardLedger in Barnard validators 7 tolerate 2

  node AlphaClient kind clearing in Alpha

  at EarthLedger.t=10.0:
      create checkpoint CE10 containing lock asset X to AlphaLedger

  at BarnardLedger.t=10.0:
      create checkpoint CB10 containing conflicting observation about X

  message CE10 from EarthLedger to AlphaClient mode laser
  message CB10 from BarnardLedger to AlphaClient mode laser

  expect:
      at AlphaLedger.t=14.4:
          knows(AlphaClient, CE10) == true
          knows(AlphaClient, CB10) == false
          may_provisionally_credit(X) == true
          may_mark_bilaterally_settled(X) == false
}
```

## I.5 relay 延迟 3 年再转发旧 checkpoint

```text
scenario relay_delays_old_checkpoint {
  frame solar_barycentric

  system Earth at (0, 0, 0) ly
  system Alpha at (4.3, 0, 0) ly

  domain EarthLedger in Earth validators 7 tolerate 2
  domain AlphaLedger in Alpha validators 7 tolerate 2

  node Relay1 kind relay in Earth
  node AlphaClient kind wallet in Alpha

  at EarthLedger.t=20.0:
      create checkpoint CE20

  message CE20 from Relay1 to AlphaClient mode laser delay 3.0 years

  expect:
      arrival_time(CE20, AlphaClient) >= 27.3
      checkpoint_usable_as_history(CE20) == true
      checkpoint_usable_for_new_credit(CE20) == false
}
```

## I.6 清算委员会签发两个冲突 checkpoint

```text
scenario clearing_committee_equivocation {
  frame local_system

  system Earth at (0,0,0) ly
  domain EarthLedger in Earth validators 10 tolerate 3

  adversary:
      EarthLedger.sign_checkpoint height 500 hash H1 quorum 7
      EarthLedger.sign_checkpoint height 500 hash H2 quorum 7
      assert H1 != H2

  expect:
      any_honest_client_receiving(H1, H2):
          detect_conflict == true
          dispute.kind == ConflictingCheckpoint
          domain_status(EarthLedger) == quarantined
}
```

## I.7 飞船携带 20 年前数据重新接入

```text
scenario ship_replays_twenty_year_old_claim {
  frame solar_barycentric

  system Earth at (0,0,0) ly
  system Alpha at (4.3,0,0) ly

  domain EarthLedger in Earth validators 7 tolerate 2
  domain AlphaLedger in Alpha validators 7 tolerate 2

  node OldShip kind ship in deep_space
  node AlphaClient kind wallet in Alpha

  ship OldShip worldline [
      at t=0.0 pos=(0,0,0),
      at t=20.0 pos=(4.3,0,0)
  ]

  at EarthLedger.t=0.1:
      create claim ClaimOld expiry 5.0

  message ClaimOld from OldShip to AlphaClient mode ship delay 20.0 years

  expect:
      AlphaClient.accepts_as_history(ClaimOld) == true
      AlphaClient.accepts_for_credit(ClaimOld) == false
      claim_status(ClaimOld) == expired
}
```

## I.8 客户端收到物理上不可能到达的信息

```text
scenario impossible_message_rejected {
  frame solar_barycentric

  system Alpha at (4.3,0,0) ly
  system Barnard at (0,5.96,0) ly

  domain AlphaLedger in Alpha validators 7 tolerate 2
  domain BarnardLedger in Barnard validators 7 tolerate 2

  node AlphaClient kind wallet in Alpha

  at BarnardLedger.t=100.0:
      create checkpoint CB100

  adversary:
      inject_message payload CB100 to AlphaClient at Alpha.t=101.0

  expect:
      lightcone_possible(CB100, AlphaClient.receive_event) == false
      AlphaClient.reject_reason == LightconeViolation
      checkpoint_store_does_not_contain(CB100)
}
```

------

# J. 关键 properties / invariants

## J.1 Safety invariants

### J.1.1 不接受违反光锥的消息

```text
Invariant LightconeValidity:

For every client C,
for every accepted proof P at receive event r,
for every dependency event e in P,
there exists a certified causal path e ->* r
such that every hop is lightcone-valid.
```

这是核心强保证。

------

### J.1.2 本地 finality 不可回滚

```text
Invariant LocalFinality:

For domain D under BFT assumption HonestQuorum(D),
there do not exist two finalized checkpoints c1, c2
such that:

c1.domain = c2.domain = D
c1.height = c2.height
hash(c1) != hash(c2)
both have valid quorum certificates.
```

这是条件强保证。
条件是本地 BFT quorum honest。

------

### J.1.3 同一 trust domain 内不能承兑两个互斥资产声明

```text
Invariant NoMutualAcceptance:

For any local domain D and asset A,
D's ledger must not contain two finalized AcceptRemoteClaim txs
for mutually exclusive claims over A,
unless both are marked Disputed before spendability.
```

这是本地 ledger 强保证。

------

### J.1.4 冲突 checkpoint 必须进入 dispute/slashing 状态

```text
Invariant ConflictDetected:

If a client observes two valid quorum-certified checkpoints
from the same domain, same height, same epoch, different hash,
then eventually:

domain_status = Quarantined
and
exists DisputeRecord(kind = ConflictingCheckpoint)
```

这是检测强保证。
slashing 是否成功执行依赖治理和抵押可执行性。

------

### J.1.5 provisional credit 不能展示为 final settlement

```text
Invariant NoFalseFinality:

For any claim c:

status(c) = ProvisionallyCredited
implies
display_status(c) != Final
and
display_status(c) includes risk_label
and
display_status(c) includes settlement_horizon.
```

这是客户端状态机强保证。

------

### J.1.6 远程未知状态不能误标为全局确认

```text
Invariant NoGlobalConfirmationClaim:

No client may display GlobalFinal
unless protocol explicitly defines a bounded observer set
and all required observer checkpoints are verified.

Default:
GlobalFinal is not a reachable state.
```

这是 UX / API 强保证。

------

## J.2 Liveness properties

### J.2.1 合法 claim 最终被处理

```text
Property ClaimEventuallyProcessed:

Assume:
  - messages are eventually delivered after light-speed lower bound
  - origin checkpoint is valid
  - destination domain has honest quorum
  - credit line capacity is sufficient
  - claim does not expire before earliest possible arrival

Then:
  destination eventually records either AcceptRemoteClaim or RejectClaim
  with a verifiable reason.
```

这是条件 liveness。

------

### J.2.2 离线客户端可重新构造可信状态

```text
Property OfflineResync:

Given:
  client has trusted anchors
  and receives sufficient checkpoint bundles
  and no unresolved conflict in required domains

Then:
  client reconstructs a unique compatible frontier
  and recomputes all relevant claim statuses.
```

------

### J.2.3 非冲突历史最终可合并

```text
Property NonConflictingHistoriesMerge:

If two checkpoint histories from different domains contain no invalid causal edge,
no same-domain fork,
and no mutually exclusive asset claim,
then client reconciliation eventually merges them into one causal mesh.
```

## J.3 Economic / risk properties

### J.3.1 信用额度限制最大敞口

```text
Invariant CreditExposureBound:

For every credit line L:

L.outstanding <= L.limit
```

进一步：

```text
MaxLoss(domain_pair) <= sum(active_credit_limits) - recoverable_collateral
```

这是风险上界，不是零风险保证。

------

### J.3.2 抵押覆盖特定作恶场景

```text
Property CollateralCoverage:

For configured scenario S,
if collateral_ratio >= required_ratio(S),
then slashable collateral >= expected_loss(S)
```

这依赖资产价格、司法执行和治理，不是纯密码学性质。

------

### J.3.3 跨光年交易必须显示风险等级和 horizon

```text
Invariant RiskDisclosure:

For every claim with status in:
  RemoteObserved,
  ProvisionallyCredited,
  AcceptedByRemoteLedger

client display must include:
  - origin domain
  - destination domain
  - known checkpoint height
  - settlement horizon
  - credit exposure
  - dispute status
  - finality level
```

------

# K. 形式化验证方案

## K.1 TLA+ / PlusCal 适合验证

适合建模：

1. 客户端状态机；
2. checkpoint 接收和冲突检测；
3. claim 生命周期；
4. provisional 到 final 的状态升级规则；
5. 异步消息投递；
6. replay / drop / reorder；
7. lightcone-validity invariant 的抽象版本；
8. credit exposure bound。

TLA+ 中不需要建模完整密码学，只把签名验证抽象为谓词：

```text
ValidQC(cp)
ValidMerkleProof(tx, cp)
ValidLightconePath(path)
```

## K.2 Alloy 适合验证

Alloy 适合关系结构检查：

1. event / message / observation 关系；
2. happens-before 是否有环；
3. lightcone relation 是否违反；
4. 某个 observation 是否可能知道某事件；
5. checkpoint DAG 是否包含非法因果边；
6. 最小 counterexample 发现。

## K.3 Coq / Isabelle / Lean 适合验证

适合做机器证明：

1. Merkle proof verifier 正确性；
2. checkpoint conflict detector soundness；
3. 状态机 promotion lattice 不会越级；
4. causal certificate verifier soundness；
5. 序列化格式 canonicalization；
6. hash / signature domain separation 规则；
7. Rust 实现核心逻辑的 refinement proof，理想情况下通过提取或验证子集完成。

## K.4 Model checking 适合验证

适合：

- 有界 validator 数；
- 有界 domain 数；
- 有界消息数；
- 有界 checkpoint 高度；
- 有界 claim 数；
- Byzantine 行为组合。

重点找最小反例。

## K.5 只能 simulation + property-based testing 的部分

以下部分很难完全形式化，只能模拟、测试和审计：

1. 经济激励是否足够；
2. 信用额度是否合理；
3. 抵押估值；
4. 政权失信；
5. relay 生态；
6. 长期密码学迁移；
7. AI 代理策略；
8. 用户是否理解风险 UI。

## K.6 从抽象模型 refinement 到实现

建议分层 refinement：

```text
Abstract TLA+ model
  ↓
Executable reference model in Python
  ↓
Rust core state machine
  ↓
Rust client library
  ↓
Integration tests with simulator
  ↓
Production node
```

保持同步的方法：

1. 所有状态枚举由同一 schema 生成；
2. TLA+ action 名称映射到 Rust transition 名称；
3. 每个 Rust transition 输出 deterministic trace；
4. trace 可导入 model checker 检查；
5. CI 中运行有界 model checking 和 replay tests。

## K.7 lightcone-validity invariant

抽象定义：

```text
Accepted(C, P, r) =>
  forall e in Dependencies(P):
      ExistsCertifiedPath(e, r)
      and
      forall hop in path:
          LightconePossible(hop.from, hop.to)
```

## K.8 causal happens-before relation

```text
HB = transitive_closure(
       LocalLedgerOrder
     ∪ MessageDelivery
     ∪ ExplicitDependency
     ∪ CheckpointObservation
)
```

性质：

```text
Acyclic(HB)
```

如果 `HB` 有环，说明出现了因果不可能或伪造 observation。

## K.9 checkpoint conflict detection

需要证明：

```text
If client has cp1 and cp2
and SameDomainSameHeightDifferentHash(cp1, cp2)
and ValidQC(cp1)
and ValidQC(cp2)
then detect_conflict(cp1, cp2) returns ConflictingCheckpoint.
```

## K.10 状态机不错误提升 final

定义偏序：

```text
Unknown < PendingLocal < LocallyFinal < ExportedToRemote
        < RemoteObserved < ProvisionallyCredited
        < AcceptedByRemoteLedger < OriginAcknowledged
        < BilaterallySettled
```

但 `Disputed`, `Expired`, `Rejected`, `Slashed` 是 absorbing 或 terminal 状态。

需要证明：

```text
No action except ObserveOriginAcknowledgement can move to OriginAcknowledged.

No action except ObserveBilateralClosure can move to BilaterallySettled.

ProvisionallyCredited cannot be displayed as Final.
```

------

# L. 最小 TLA+ / Alloy 草案

## L.1 TLA+ 风格模型草案

```tla
----------------------------- MODULE InterstellarLedger -----------------------------
EXTENDS Naturals, FiniteSets, Sequences

CONSTANTS
  Domains,
  Clients,
  Assets,
  Claims,
  Heights,
  Times,
  Dist,
  LightDelay,
  ByzantineDomains

VARIABLES
  time,
  ledgers,
  checkpoints,
  messages,
  known,
  status,
  disputes,
  creditOutstanding,
  creditLimit,
  quarantined

ClaimStatuses ==
  {"Unknown",
   "PendingLocal",
   "LocallyFinal",
   "ExportedToRemote",
   "RemoteObserved",
   "ProvisionallyCredited",
   "AcceptedByRemoteLedger",
   "OriginAcknowledged",
   "BilaterallySettled",
   "Disputed",
   "Expired",
   "Slashed",
   "Rejected"}

TypeOK ==
  /\ status \in [Claims -> ClaimStatuses]
  /\ quarantined \subseteq Domains
  /\ creditOutstanding \in [Domains \X Domains -> Nat]
  /\ creditLimit \in [Domains \X Domains -> Nat]

LightconePossible(e1, e2) ==
  e2.time >= e1.time + LightDelay[e1.domain][e2.domain]

ValidQC(cp) ==
  cp.validQC = TRUE

SameHeightConflict(cp1, cp2) ==
  /\ cp1.domain = cp2.domain
  /\ cp1.height = cp2.height
  /\ cp1.hash # cp2.hash
  /\ ValidQC(cp1)
  /\ ValidQC(cp2)

Init ==
  /\ time = 0
  /\ ledgers = [d \in Domains |-> << >>]
  /\ checkpoints = {}
  /\ messages = {}
  /\ known = [c \in Clients |-> {}]
  /\ status = [cl \in Claims |-> "Unknown"]
  /\ disputes = {}
  /\ creditOutstanding = [p \in Domains \X Domains |-> 0]
  /\ creditLimit = [p \in Domains \X Domains |-> 0]
  /\ quarantined = {}

LocalCommit(d, tx) ==
  /\ d \in Domains
  /\ ledgers' = [ledgers EXCEPT ![d] = Append(@, tx)]
  /\ UNCHANGED <<time, checkpoints, messages, known, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

MakeCheckpoint(d, cp) ==
  /\ d \in Domains
  /\ cp.domain = d
  /\ ValidQC(cp)
  /\ checkpoints' = checkpoints \cup {cp}
  /\ UNCHANGED <<time, ledgers, messages, known, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

SendMessage(m) ==
  /\ m.sendTime >= time
  /\ messages' = messages \cup {m}
  /\ UNCHANGED <<time, ledgers, checkpoints, known, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

DeliverMessage(c, m) ==
  /\ m \in messages
  /\ time >= m.sendTime + LightDelay[m.fromDomain][m.toDomain]
  /\ known' = [known EXCEPT ![c] = @ \cup {m.payload}]
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

RejectImpossibleMessage(c, m) ==
  /\ m \in messages
  /\ time < m.sendTime + LightDelay[m.fromDomain][m.toDomain]
  /\ known' = known
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

AcceptClaim(c, cl, fromD, toD, amount) ==
  /\ status[cl] = "RemoteObserved"
  /\ creditOutstanding[fromD, toD] + amount <= creditLimit[fromD, toD]
  /\ status' = [status EXCEPT ![cl] = "ProvisionallyCredited"]
  /\ creditOutstanding' =
        [creditOutstanding EXCEPT ![fromD, toD] = @ + amount]
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, known,
                 disputes, creditLimit, quarantined>>

ObserveRemoteReceipt(cl) ==
  /\ status[cl] = "ProvisionallyCredited"
  /\ status' = [status EXCEPT ![cl] = "AcceptedByRemoteLedger"]
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, known,
                 disputes, creditOutstanding, creditLimit, quarantined>>

ObserveOriginAck(cl) ==
  /\ status[cl] = "AcceptedByRemoteLedger"
  /\ status' = [status EXCEPT ![cl] = "OriginAcknowledged"]
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, known,
                 disputes, creditOutstanding, creditLimit, quarantined>>

ObserveBilateralClosure(cl) ==
  /\ status[cl] = "OriginAcknowledged"
  /\ status' = [status EXCEPT ![cl] = "BilaterallySettled"]
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, known,
                 disputes, creditOutstanding, creditLimit, quarantined>>

DetectConflict(cp1, cp2) ==
  /\ cp1 \in checkpoints
  /\ cp2 \in checkpoints
  /\ SameHeightConflict(cp1, cp2)
  /\ disputes' = disputes \cup {{cp1, cp2}}
  /\ quarantined' = quarantined \cup {cp1.domain}
  /\ UNCHANGED <<time, ledgers, checkpoints, messages, known,
                 status, creditOutstanding, creditLimit>>

Tick ==
  /\ time' = time + 1
  /\ UNCHANGED <<ledgers, checkpoints, messages, known, status,
                 disputes, creditOutstanding, creditLimit, quarantined>>

Next ==
  \/ \E d \in Domains, tx \in Assets: LocalCommit(d, tx)
  \/ \E d \in Domains, cp: MakeCheckpoint(d, cp)
  \/ \E m: SendMessage(m)
  \/ \E c \in Clients, m \in messages: DeliverMessage(c, m)
  \/ \E c \in Clients, m \in messages: RejectImpossibleMessage(c, m)
  \/ \E c \in Clients, cl \in Claims, d1 \in Domains, d2 \in Domains, a \in Nat:
        AcceptClaim(c, cl, d1, d2, a)
  \/ \E cl \in Claims: ObserveRemoteReceipt(cl)
  \/ \E cl \in Claims: ObserveOriginAck(cl)
  \/ \E cl \in Claims: ObserveBilateralClosure(cl)
  \/ \E cp1 \in checkpoints, cp2 \in checkpoints: DetectConflict(cp1, cp2)
  \/ Tick

CreditBound ==
  \A d1 \in Domains, d2 \in Domains:
      creditOutstanding[d1, d2] <= creditLimit[d1, d2]

NoFalseFinality ==
  \A cl \in Claims:
      status[cl] = "ProvisionallyCredited" => status[cl] # "BilaterallySettled"

ConflictQuarantine ==
  \A cp1 \in checkpoints, cp2 \in checkpoints:
      SameHeightConflict(cp1, cp2) => cp1.domain \in quarantined

Spec ==
  Init /\ [][Next]_<<time, ledgers, checkpoints, messages, known, status,
             disputes, creditOutstanding, creditLimit, quarantined>>

THEOREM Spec => []TypeOK
THEOREM Spec => []CreditBound
THEOREM Spec => []NoFalseFinality
=============================================================================
```

这个模型故意抽象掉了真实密码学和完整地理坐标，只保留状态机、消息延迟和关键 invariant。

## L.2 Alloy 风格关系模型草案

```alloy
module InterstellarCausality

open util/integer

sig Domain {}

sig Event {
  domain: one Domain,
  time: one Int,
  deps: set Event
}

sig Message {
  from: one Event,
  to: one Event,
  sendTime: one Int,
  receiveTime: one Int
}

sig Checkpoint {
  domain: one Domain,
  height: one Int,
  hash: one Int,
  observes: set Checkpoint,
  event: one Event,
  validQC: one Bool
}

one sig TrueBool, FalseBool extends Bool {}
abstract sig Bool {}

fun dist[d1: Domain, d2: Domain]: Int {
  // bounded model: supplied by scenario
  0
}

pred LightconePossible[e1: Event, e2: Event] {
  e2.time >= e1.time.plus[dist[e1.domain, e2.domain]]
}

pred MessageValid[m: Message] {
  m.receiveTime >= m.sendTime.plus[dist[m.from.domain, m.to.domain]]
  LightconePossible[m.from, m.to]
}

fact DependenciesRespectLightcone {
  all e: Event, d: e.deps |
    LightconePossible[d, e]
}

fact MessagesRespectLightcone {
  all m: Message |
    MessageValid[m]
}

pred SameHeightConflict[c1, c2: Checkpoint] {
  c1.domain = c2.domain
  c1.height = c2.height
  c1.hash != c2.hash
  c1.validQC = TrueBool
  c2.validQC = TrueBool
}

assert NoImpossibleDependency {
  all e: Event, d: e.deps |
    LightconePossible[d, e]
}

assert NoCausalCycle {
  no e: Event | e in e.^deps
}

check NoImpossibleDependency for 5 Domain, 20 Event, 10 Message, 10 Checkpoint
check NoCausalCycle for 5 Domain, 20 Event
```

实际 Alloy 模型应把 `dist` 作为 scenario relation 输入，而不是固定函数。

------

# M. 工程路线图：MVP、研究原型、生产级系统

## M.1 推荐语言

| 语言                      | 适合模块                                                     |
| ------------------------- | ------------------------------------------------------------ |
| **Rust**                  | 客户端核心、状态机、证明验证、序列化、加密、light client、validator critical path |
| **Go**                    | relay、archive service、运维工具、network daemon             |
| **Python**                | spacetime simulator、scenario DSL、property-based testing、trace analysis |
| **TypeScript**            | 钱包 UI、risk dashboard、explorer、operator console          |
| **OCaml**                 | 可选：高可靠协议原型、形式化友好的 reference implementation  |
| **Lean / Coq / Isabelle** | 证明核心 verifier、状态机 invariant、Merkle proof 正确性     |

客户端核心建议用 Rust。
理由：

- 内存安全；
- 性能足够；
- 适合长期维护；
- 生态中有成熟加密和序列化库；
- 可与 fuzzing、property testing、WASM UI 集成；
- 可将核心 verifier 编译到 wallet、relay、archive、browser extension。

## M.2 模块划分

```text
crates/
  chrono-core/
    types
    hashing
    canonical encoding
    state machine
    status lattice

  chrono-crypto/
    threshold signatures
    PQ signatures
    hash agility
    domain separation

  chrono-ledger/
    local transaction validation
    asset model
    export lock
    import claim
    dispute rules

  chrono-client/
    sync engine
    light client verifier
    claim manager
    risk policy
    offline resync

  chrono-causal/
    causal certificate verifier
    lightcone proof verifier
    coordinate model

  chrono-clearing/
    credit line
    haircut
    collateral
    settlement horizon

  chrono-net/
    relay protocol
    DTN bundles
    anti-replay
    peer diversity

  chrono-testbench/
    simulator bindings
    deterministic replay
    fuzz harness

  chrono-ffi/
    TypeScript / Python / C bindings
```

## M.3 MVP

目标：证明核心概念可运行。

范围：

1. 两到三个 domain；
2. 固定 validator set；
3. 本地 BFT 可先抽象为 threshold-signed block；
4. 支持 object asset；
5. 支持 `LockForExport`；
6. 支持 `SettlementClaim`；
7. 支持远方 `ProvisionallyCredited`；
8. 支持 checkpoint conflict detection；
9. 支持 lightcone simulator；
10. 支持测试 DSL；
11. 支持 claim 状态 UI。

MVP 不做：

- 复杂智能合约；
- permissionless PoS；
- 真实经济抵押；
- 完整 PQ migration；
- 多层本地分片。

## M.4 研究原型

加入：

1. 真实 BFT consensus，例如 HotStuff-like 或 Tendermint-like；
2. validator epoch transition；
3. threshold signature aggregation；
4. sparse vector frontier；
5. causal certificate minimization；
6. real DTN relay；
7. checkpoint gossip；
8. credit line 和 haircut 模型；
9. insurance pool prototype；
10. TLA+ model checking；
11. Alloy causal consistency checking；
12. Rust fuzzing；
13. deterministic replay；
14. adversarial simulation。

## M.5 生产级系统

生产级需要：

1. 多实现客户端；
2. 独立 archive 网络；
3. watchdog / watcher 生态；
4. HSM / MPC key management；
5. governance-defined validator admission；
6. slashing escrow；
7. dispute court / arbitration process；
8. protocol version negotiation；
9. long-term archival media；
10. cryptographic era migration；
11. PQ hybrid signatures；
12. proof compression；
13. operator audit logs；
14. legal wrapper for credit claims；
15. UI 强制风险披露；
16. disaster recovery and fork governance。

## M.6 CI 设计

CI pipeline：

```text
1. Unit tests
   - parser
   - hashing
   - signature verification
   - Merkle proof
   - status lattice

2. Integration tests
   - local ledger
   - checkpoint creation
   - claim creation
   - remote verification
   - dispute flow

3. Spacetime simulation tests
   - lightcone validity
   - delayed relay
   - shipborne replay
   - impossible message rejection

4. Property-based tests
   - random checkpoint DAG
   - random settlement lifecycle
   - random credit lines
   - random offline intervals

5. Fuzz tests
   - proof bundles
   - network messages
   - checkpoint parser
   - causal certificate parser

6. Model checking
   - TLA+ bounded runs
   - Alloy relation checks

7. Deterministic replay
   - every failing seed saved
   - every regression replayed

8. UI state tests
   - provisional never shown as final
   - disputed always visible
   - settlement horizon always present
```

## M.7 协议版本化与长期密码学升级

每个 checkpoint 包含：

```text
protocol_version
crypto_suite_id
previous_crypto_suite_id
migration_policy_hash
```

长期升级规则：

1. 新老算法并行签名一个 era；
2. checkpoint 中记录 crypto transition；
3. 老资产必须在 deadline 前迁移；
4. 长期 claim 使用 hash renewal；
5. archive 节点周期性 re-anchor 历史；
6. 客户端拒绝过期 crypto suite 支撑的新信用；
7. 历史证明可验证，但新价值转移必须使用当前 suite。

------

# N. 哪些地方仍然不可避免依赖信用 / 治理，而不是技术

这个系统的底线是：

> 密码学能证明“谁签了什么、何时可能知道什么、某资产是否在某本地账本中被锁定”。
> 密码学不能消除跨光年信用风险。

不可避免依赖信用或治理的部分包括：

1. **本地 validator set 的合法性**
   - 谁能成为 validator？
   - 本地政权是否能强制替换 validator？
   - 这是治理问题。
2. **本地 quorum 集体作恶后的赔偿**
   - 可以产生 slashing evidence。
   - 但能否罚没抵押取决于抵押所在地和执行机制。
3. **信用额度**
   - B 星系愿意给 A 星系多少信用，不是密码学问题。
   - 它依赖历史信誉、贸易关系、政治风险和保险。
4. **抵押品估值**
   - 跨星系抵押品可能多年后才可执行。
   - 价格、流动性、可扣押性都是经济问题。
5. **保险池**
   - 保险池是否足够资本化；
   - 是否会系统性违约；
   - 这些不是 ledger 本身能保证的。
6. **物理坐标 attestation**
   - 光锥验证需要坐标和时间证明。
   - 观测站、导航信标、船舶日志本身需要信任、冗余和审计。
7. **跨政权 dispute resolution**
   - 哪个法院、仲裁机构或治理协议有权裁决？
   - 技术只能提交证据。
8. **长期密码学破坏**
   - 可以设计升级路径。
   - 不能保证未来数学或物理发现不会破坏旧算法。
9. **AI 金融代理行为**
   - AI 可以执行策略。
   - 风险偏好、异常判断和责任归属仍然是治理问题。
10. **全银河最终性**
    - 本系统不承诺。
    - 任何声称“全银河实时最终确认”的系统都违反物理现实。

------

## 最终简化版原则

这个系统只坚持三条硬原则：

```text
1. Local serializability:
   稀缺资产在 home domain 内必须有本地 total order。

2. Global causality:
   跨星系证明必须遵守光锥，不能接受物理上不可能知道的信息。

3. Explicit risk:
   跨星系承兑是债权和信用，不是假装即时最终结算。
```

也就是：

> **本地账本负责强一致，跨星系 checkpoint 负责证明，credit network 负责承担和定价风险。**

可以，把上一版设计里的密码层改成一个独立的 **Cryptographic Survival Layer / 宇宙尺度密码生存层**。核心原则是：

> **不要把任何具体算法设计成永久安全；只把“可替换、可多样化、可重锚定、可证明在某个时代有效”设计成永久协议规则。**

下面是对原方案的补充。

------

# 1. 宇宙尺度下，密码安全要重新定义

传统系统常默认：

```text
某算法今天安全 → 足够用很多年
```

星际账本不能这么做。原因是：

1. **消息飞行时间可能长达数年到数十年。**
   一个 checkpoint 在 A 星系签发时算法还安全，等它到达 B 星系时，算法可能已经被破解、削弱或废弃。
2. **历史证明要保存几十年、几百年甚至更久。**
   普通区块链把历史签名当成永久证据；星际账本不能这么乐观。
3. **敌手可以“收割现在，未来破解”。**
   所有星际广播都应假设会被长期记录。后量子迁移的核心动机之一就是未来大规模量子计算机可能破坏现有公钥算法，因此不能把 RSA、ECDSA、EdDSA、BLS 作为长期安全基础。IETF 的 PQC 工程草案也把 CRQC，即 cryptographically relevant quantum computer，对现有传统公钥算法的威胁作为迁移动机。([IETF](https://www.ietf.org/archive/id/draft-ietf-pquip-pqc-engineers-11.html))
4. **算法会经历未知密码分析。**
   后量子算法不是“物理定律保证安全”，而是目前基于数学困难性假设。NIST 已标准化 ML-KEM、ML-DSA、SLH-DSA，并继续保留备用路线，例如 HQC 作为 ML-KEM 的备份方向，这本身就说明不能押注单一数学家族。([NIST](https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards))

因此，星际账本中的证明应写成：

```text
Proof validity is relative to:
  - cryptographic era
  - observer spacetime position
  - algorithm status at observation time
  - renewal / re-anchoring history
```

也就是：

> 不是“这个签名永远有效”，而是“这个对象在第 k 个密码时代被有效签名，并且在算法退役前被后续时代重新锚定”。

------

# 2. 默认密码选择：不要用单一算法族

截至当前标准化状态，工程上合理的默认路线是：

| 用途                 | 推荐主算法                                       | 备用 / 多样化算法                    | 说明                                                         |
| -------------------- | ------------------------------------------------ | ------------------------------------ | ------------------------------------------------------------ |
| 节点间密钥交换       | ML-KEM-768 / ML-KEM-1024                         | HQC，待其标准完成后加入              | ML-KEM 是 NIST FIPS 203 标准化 KEM；HQC 被 NIST 选为 ML-KEM 的备份方向，草案预计随后推进。([NIST Computer Security Resource Center](https://csrc.nist.gov/pubs/fips/203/final)) |
| 普通交易签名         | ML-DSA                                           | SLH-DSA 用于高价值或低频签名         | ML-DSA 是 NIST FIPS 204 标准化签名算法，NIST 称其被认为可抵抗拥有大规模量子计算机的敌手。([NIST Computer Security Resource Center](https://csrc.nist.gov/pubs/fips/204/final)) |
| 长期 checkpoint 签名 | ML-DSA + SLH-DSA 双签                            | 未来加入 FN-DSA / 其他成熟方案       | SLH-DSA 是无状态哈希签名，基于 SPHINCS+，适合作为不同数学基础的长期备份。([NIST Computer Security Resource Center](https://csrc.nist.gov/pubs/fips/205/final)) |
| 对称加密             | AES-256 或 ChaCha20/XChaCha20 级别               | 定期重加密                           | NIST 认为 AES-192 和 AES-256 在量子威胁下仍可长期安全，但这假设没有新的经典或量子密码分析突破。([NIST Computer Security Resource Center](https://csrc.nist.gov/projects/post-quantum-cryptography/faqs)) |
| 哈希 / Merkle root   | SHA-384 / SHA-512 / SHAKE256 级别                | 多哈希承诺                           | 跨世纪账本不要只用 256-bit hash 作为唯一长期根。             |
| 随机数               | 本地量子 RNG + 多源熵池 + 阈值 randomness beacon | 物理熵只作为输入，必须经过 extractor | 物理随机性有用，但不能把未经建模的物理噪声直接当密钥。       |

关键点：

> **本地高频交易可以使用一种快速 PQ 签名；跨星系 checkpoint 和长期债权证明必须使用算法多样化签名束。**

------

# 3. 不建议继续依赖的东西

## 3.1 不把 ECDSA / EdDSA / BLS 当长期安全基础

它们可以作为短期兼容层、legacy identity 或本地临时认证，但不能作为以下对象的唯一安全依据：

```text
- checkpoint quorum certificate
- export lock proof
- settlement claim
- credit line authorization
- dispute / slashing evidence
- long-term asset ownership proof
```

原因很直接：椭圆曲线和有限域离散对数体系是大规模量子计算机的典型攻击目标。

## 3.2 不把 BLS 聚合签名作为 checkpoint 的最终证明

BLS 聚合签名很适合今天的区块链，因为它短、小、可聚合。但在星际长期证明里，它的问题是：

```text
BLS 是 pairing-based ECC。
ECC 一旦被 CRQC 破坏，历史 BLS quorum certificate 就会失去长期证明力。
```

所以可以允许：

```text
BLS / Ed25519 / secp256k1:
  only as local fast-path hint
  not as archival validity root
```

真正写入长期 checkpoint 的必须是：

```text
PQ SignatureBundle
```

例如：

```rust
struct SignatureBundle {
    object_hash: Hash,

    signatures: Vec<IndividualSignature>,

    quorum_policy: QuorumPolicy,

    crypto_era_id: CryptoEraId,

    algorithms_required: Vec<CryptoSuiteId>,

    merkle_root_of_signatures: Hash,
}
```

------

# 4. PQ threshold signature：不要假设它已经像 BLS 一样成熟

在上一版设计中我提到了 threshold signature。这里需要修正为更保守的工程设计：

> **不要把“高效、短小、标准化、后量子 threshold aggregate signature”作为 MVP 依赖。**

后量子签名可以先这样做：

```text
Validator quorum certificate =
  Merkleized set of individual PQ signatures
```

也就是：

```rust
struct PQQuorumCertificate {
    domain_id: DomainId,
    epoch: u64,
    message_hash: Hash,

    threshold_numerator: u32,
    threshold_denominator: u32,

    signer_bitmap: Vec<u8>,

    signatures_root: Hash,
    included_signatures: Vec<IndividualSignature>,

    crypto_era_id: CryptoEraId,
}
```

跨星系通信带宽慢，但交易频率低，所以 checkpoint 大一点可以接受。
星际账本的优先级是：

```text
verifiability > compactness
survivability > throughput
algorithmic diversity > elegance
```

未来如果有成熟 PQ 聚合签名，可以作为新 crypto era 引入，但不能把它作为系统正确性的前提。

------

# 5. Cryptographic Era：把算法生命周期写进协议

必须引入一个协议级对象：

```rust
struct CryptoEra {
    era_id: CryptoEraId,

    valid_from_coord: SpacetimeCoord,

    soft_deprecate_after: Option<SpacetimeCoord>,
    hard_reject_after: Option<SpacetimeCoord>,

    signature_suites: Vec<CryptoSuiteId>,
    kem_suites: Vec<CryptoSuiteId>,
    hash_suites: Vec<HashSuiteId>,
    aead_suites: Vec<AeadSuiteId>,

    required_for_checkpoint: ThresholdPolicy,
    required_for_settlement_claim: ThresholdPolicy,
    required_for_credit_line: ThresholdPolicy,

    renewal_policy: RenewalPolicy,

    governance_certificate: SignatureBundle,
}
```

每个 checkpoint 必须声明：

```rust
struct Checkpoint {
    ...
    crypto_era_id: CryptoEraId,
    hash_suite_id: HashSuiteId,
    signature_bundle: SignatureBundle,
    previous_era_anchor: Option<Hash>,
}
```

客户端验证时不是简单地问：

```text
signature valid?
```

而是问：

```text
signature valid under which crypto era?
was this era active at the claimed event?
has this proof been renewed before the era expired?
is the receiving observer allowed to rely on this era now?
```

------

# 6. Re-anchoring / hash renewal：历史必须周期性续命

星际账本需要一个“历史续命”协议。

## 6.1 为什么需要 re-anchoring

假设：

```text
Year 2200: Earth 签发 checkpoint C using ML-DSA
Year 2230: C 到达 distant colony
Year 2225: ML-DSA 出现严重密码分析突破
```

那么 B 星系在 2230 年收到 C 时不能简单接受它。因为它无法排除：

```text
攻击者在 2225 年之后伪造了一个看似来自 2200 年的 checkpoint。
```

除非 C 有一个更强证据链证明：

```text
C 在 2225 年之前已经被某些独立观察者看到、记录，并在新算法下重新签名。
```

## 6.2 RenewalCheckpoint

```rust
struct RenewalCheckpoint {
    renewal_id: Hash,

    old_checkpoint_root: Hash,
    old_crypto_era_id: CryptoEraId,

    new_crypto_era_id: CryptoEraId,

    renewed_at_event: EventId,
    renewed_at_coord: SpacetimeCoord,

    witness_domains: Vec<DomainId>,

    old_proof_bundle_hash: Hash,

    new_hash_commitment: MultiHashCommitment,

    new_signature_bundle: SignatureBundle,

    causal_certificate: CausalCertificate,
    lightcone_proof: LightconeProof,
}
```

## 6.3 验证规则

```python
def verify_archival_proof(object, observer_event):
    chain = object.renewal_chain

    for step in chain:
        assert verify_signature_bundle(step.new_signature_bundle)
        assert verify_lightcone_proof(step.lightcone_proof)
        assert step.renewed_at_coord <= step.old_era.hard_reject_after

    assert latest_era_is_acceptable(chain[-1].new_crypto_era_id, observer_event)

    return Accept
```

核心 invariant：

```text
A historical proof is acceptable only if every cryptographic transition
was anchored before the previous era became untrustworthy.
```

这比“验证旧签名”更强，也更适合跨世纪系统。

------

# 7. 多算法承诺：不要只 hash 一次

所有长期对象应使用 `MultiHashCommitment`：

```rust
struct MultiHashCommitment {
    object_canonical_encoding_hashes: Vec<HashDigest>
}

struct HashDigest {
    suite_id: HashSuiteId,
    digest: Vec<u8>,
}
```

例如 checkpoint root 不只是：

```text
SHA-256(object)
```

而是：

```text
SHA-384(object)
SHA-512(object)
SHAKE256(object, 512 bits)
BLAKE3-512 or future-approved hash
```

验证规则：

```text
current era requires at least k-of-n hash digests to match
```

不要为了“简单”使用太多算法，但至少要避免单一哈希失败导致整个历史证明失效。KISS 版本可以是：

```text
MVP:
  SHA-512/384 + SHAKE256

Production:
  era-governed multi-hash commitment
```

------

# 8. 加密通信：KEM 要混合，不要单独押 ML-KEM

节点间建立会话密钥可以使用：

```text
HybridKEM:
  shared_secret = KDF(
      ML-KEM secret
   || classical ECDHE secret, transitional only
   || optional HQC secret once standardized
   || optional physically delivered secret
   || transcript_hash
  )
```

NIST FIPS 203 标准化的是 ML-KEM，参数集包括 ML-KEM-512、768、1024；HQC 已被 NIST 选为 ML-KEM 的后备加密算法方向，因为它基于不同数学路线。([NIST Computer Security Resource Center](https://csrc.nist.gov/pubs/fips/203/final))
IETF 的 TLS 混合密钥交换草案也体现了工程迁移思路：把 ML-KEM 与传统 ECDHE 组合，从而在迁移期降低单算法失败风险。([IETF Datatracker](https://datatracker.ietf.org/doc/draft-ietf-tls-ecdhe-mlkem/?utm_source=chatgpt.com))

对于星际账本，建议：

```text
Local routine traffic:
  ML-KEM-768 + X25519 transitional hybrid

High-value interstellar settlement:
  ML-KEM-1024 + algorithmically independent backup KEM
  + physically pre-shared / courier-delivered key material if available

Archival encrypted payload:
  symmetric data key wrapped under multiple KEMs
  periodic re-wrapping under new crypto era
```

注意：ledger proof 本身最好公开可验证。
需要长期保密的数据，例如商业合同明文、身份映射、AI 代理策略、私有信用额度，才需要加密。

------

# 9. 长期保密：PQC 不等于永远保密

对跨星际系统，机密性比完整性更难。

完整性可以通过：

```text
signature renewal
hash renewal
checkpoint re-anchoring
```

持续延长。

但保密性一旦 ciphertext 被广播并存档，就无法“撤回”。如果加密算法未来被破，旧密文就可能泄露。

因此要区分：

| 数据类型         | 建议                                 |
| ---------------- | ------------------------------------ |
| 账本 checkpoint  | 公开，不依赖保密                     |
| Merkle proof     | 公开                                 |
| Settlement claim | 大部分公开；敏感字段可承诺化         |
| 信用额度细节     | 可选择加密，但必须支持未来重加密     |
| 商业合同明文     | 不要直接上链；上链 hash / commitment |
| 身份映射         | 分层加密，定期 rewrap                |
| 私钥材料         | 永不广播；MPC/HSM/物理隔离           |

对于极高价值、低带宽、长期保密数据，可以使用：

```text
physical one-time pad courier
```

这在理论上非常强，但工程代价巨大：

```text
- key 必须真随机
- key 长度必须至少等于消息长度
- key 只能使用一次
- key 必须物理保密传递
- key 丢失则数据不可恢复
```

这适合：

```text
外交密钥
舰队指令
极高价值清算主密钥
```

不适合作为普通账本通信基础。

------

# 10. 物理定律能辅助什么，不能辅助什么

## 10.1 可以强力辅助：光速限制 / 相对论因果

这是上一版系统的核心。它可以提供：

```text
- 某节点不可能已经知道某消息
- 某 checkpoint 不可能已经观察某远方事件
- 某承兑行为发生时，冲突证据尚不在其过去光锥内
```

这不是传统密码学，而是物理约束。

可增加一个字段：

```rust
struct PhysicalSecurityAssumption {
    assumption_id: Hash,
    kind: PhysicalAssumptionKind,
    parameters: Vec<u8>,
    validity_region: Region3D,
    evidence: Vec<Observation>,
}

enum PhysicalAssumptionKind {
    NoSuperluminalSignalling,
    BoundedRelayVelocity,
    ObservatoryTimeAttestation,
    SeparatedSignerNonCommunication,
    QuantumRandomnessSource,
}
```

其中最重要的是：

```text
NoSuperluminalSignalling
```

这个可以作为强协议假设。
如果宇宙允许超光速通信，整个系统模型就失效；但在已知物理下这是最稳的假设之一。

## 10.2 可以辅助：相对论承诺 / split-agent protocol

可以设计一种 **relativistic anti-equivocation witness**：

```text
一个 validator 的 signing authority 被拆成多个空间分离的代理。
某些签名必须在短时间窗口内由多个代理分别响应。
由于代理之间不能超光速通信，它们无法在收到挑战后协调伪造两个互斥响应。
```

适合：

```text
- 高价值 checkpoint ceremony
- dispute challenge
- validator liveness proof
- anti-equivocation challenge
```

限制：

```text
- 如果攻击者提前预签两个分支，仍可能作恶
- 如果所有 split agents 被同一政权控制，治理风险仍在
- 不适合高频共识
```

所以它是辅助机制，不是替代 BFT 或签名。

## 10.3 可以辅助：pulsar / VLBI / 天文时间锚

物理世界可以帮助证明：

```text
某事件发生在某个 spacetime region
```

可以使用：

```text
- pulsar timing observations
- local atomic clocks
- VLBI-style baseline measurements
- multi-observatory timestamp attestations
- stellar occultation / known ephemeris events
```

这些不是密钥，也不是随机数，而是：

```text
spacetime attestation inputs
```

它们服务于：

```text
lightcone proof
causal certificate
checkpoint freshness
anti-backdating
```

数据结构：

```rust
struct TimePositionAttestation {
    attester_id: PublicKey,
    observatory_domain: DomainId,

    claimed_coord: SpacetimeCoord,

    method: AttestationMethod,

    raw_observation_commitment: Hash,

    error_bound: f64,

    signature_bundle: SignatureBundle,
}

enum AttestationMethod {
    AtomicClock,
    PulsarTiming,
    VLBI,
    OpticalBeacon,
    ShipNavigationLog,
    MultiStationTriangulation,
}
```

注意：这不是绝对可信。
它依赖观测站诚实、测量误差模型和多源交叉验证。

## 10.4 可以辅助：量子随机数

量子随机数发生器适合作为熵源：

```text
entropy_pool = Extract(
    qrng_output
 || hardware_rng
 || environmental_noise
 || threshold_beacon
 || operator_entropy
)
```

但是：

```text
不能直接把原始物理噪声当密钥
必须经过 randomness extractor
必须检测设备故障 / 偏置 / 注入攻击
```

建议加入：

```rust
struct EntropyAttestation {
    source_id: PublicKey,
    source_kind: EntropySourceKind,
    min_entropy_estimate: u32,
    health_tests_root: Hash,
    extraction_algorithm: Hash,
    output_commitment: Hash,
    signature_bundle: SignatureBundle,
}
```

## 10.5 谨慎使用：QKD

QKD 不能作为星际账本的主安全基础。

原因：

1. QKD 只解决密钥分发的一部分，不自动提供身份认证；NSA 明确指出 QKD 需要额外认证机制，而且其安全高度依赖硬件和工程实现。([National Security Agency](https://www.nsa.gov/Cybersecurity/Quantum-Key-Distribution-QKD-and-Quantum-Cryptography-QC/))
2. 长距离 QKD 往往需要 trusted relay；卫星 QKD 研究也承认，现有技术下扩展到全球尺度通常依赖 trusted-node 模式，而信任节点会持有经典密钥材料。([Springer](https://link.springer.com/article/10.1140/epjqt/s40507-025-00354-1))
3. 星际距离下，量子信号损耗、指向、存储、量子中继和硬件寿命都比行星尺度困难得多。
4. QKD 对 DoS 很敏感；攻击者可以破坏链路而不必破解密码。

所以建议：

```text
QKD:
  allowed as optional local / interplanetary key material source
  never required for ledger validity
  never replaces PQ signatures
  never replaces causal/lightcone verification
```

正确组合方式：

```text
session_key = KDF(
    PQ_KEM_secret
 || QKD_key_material_if_available
 || transcript_hash
)
```

也就是：

> QKD 可以加分，但不能成为唯一安全来源。

## 10.6 不要使用：物理常数作为秘密

这些东西不能当密钥：

```text
π
e
精细结构常数
CMB 公开观测值
公开 pulsar ephemeris
公开天文事件
```

因为它们是公共信息。
它们可以用作：

```text
domain separator
public randomness beacon component
timestamp challenge
coordinate reference
```

但不能用作秘密。

------

# 11. 新增证明状态：crypto-current / crypto-stale / crypto-renewed

客户端 UI 不应只显示：

```text
locally-final
remote-observed
bilaterally-settled
```

还应显示密码健康状态：

```text
CryptoStatus =
  CryptoCurrent
  CryptoDeprecatedButRenewed
  CryptoStaleNeedsRenewal
  CryptoBrokenUntrusted
  CryptoUnknownSuite
  PhysicalAnchorOnly
```

例如一笔 settlement claim 可以同时是：

```text
claim_status: remote-observed
finality_status: provisionally-credited
crypto_status: crypto-stale-needs-renewal
lightcone_status: valid
risk_status: high
```

这很重要。
一个远方 claim 的 Merkle proof 可以逻辑正确，但其签名算法可能已经过期。客户端必须把这两个维度分开。

------

# 12. 更新后的 checkpoint 数据结构

```rust
struct Checkpoint {
    domain_id: DomainId,
    height: u64,
    epoch: u64,

    prev_checkpoint_hash: Option<CheckpointHash>,

    roots: CheckpointRoots,

    coord: SpacetimeCoord,

    crypto_era_id: CryptoEraId,

    multi_hash: MultiHashCommitment,

    quorum_certificate_bundle: PQQuorumCertificateBundle,

    archival_signature_bundle: Option<SignatureBundle>,

    renewal_parent: Option<Hash>,

    observed_remote_root: Hash,

    causal_certificate_root: Hash,

    protocol_version: u32,
}
```

其中：

```rust
struct PQQuorumCertificateBundle {
    fast_path: Option<PQQuorumCertificate>,      // e.g. ML-DSA quorum
    archival_path: Option<PQQuorumCertificate>,  // e.g. SLH-DSA quorum
    legacy_hint: Option<LegacyCertificate>,      // e.g. BLS, not authoritative
    required_policy: QuorumBundlePolicy,
}

enum QuorumBundlePolicy {
    RequireAll(Vec<CryptoSuiteId>),
    RequireKOfN { k: u32, suites: Vec<CryptoSuiteId> },
    RequireFastNowArchivalLater {
        fast: CryptoSuiteId,
        archival_deadline: SpacetimeCoord,
    },
}
```

------

# 13. 更新后的验证伪代码

```python
def verify_checkpoint_crypto(client, checkpoint, observer_event):
    era = client.crypto_registry.get(checkpoint.crypto_era_id)

    if era is None:
        return Reject("unknown crypto era")

    if observer_event.coord.time.min > era.hard_reject_after:
        if not has_valid_renewal_chain(checkpoint, observer_event):
            return Reject("crypto era expired without renewal")

    if not verify_multihash(checkpoint.multi_hash, checkpoint.canonical_bytes):
        return Reject("multi-hash mismatch")

    if not verify_quorum_bundle(
        checkpoint.quorum_certificate_bundle,
        checkpoint.multi_hash,
        era.required_for_checkpoint
    ):
        return Reject("invalid PQ quorum bundle")

    if checkpoint.archival_signature_bundle is not None:
        if not verify_signature_bundle(
            checkpoint.archival_signature_bundle,
            checkpoint.multi_hash,
            observer_event
        ):
            return Reject("invalid archival signature")

    return Accept("crypto-current-or-renewed")
```

Settlement claim 验证也要加：

```python
def verify_settlement_claim_crypto(client, claim, observer_event):
    origin_cp = claim.origin_checkpoint

    cp_crypto = verify_checkpoint_crypto(client, origin_cp, observer_event)
    if not cp_crypto.accepted:
        return Reject(cp_crypto.reason)

    claim_sig_ok = verify_signature_bundle(
        claim.signature_bundle,
        claim.claim_id,
        observer_event
    )

    if not claim_sig_ok:
        return Reject("claim signature invalid or crypto-stale")

    if claim.crypto_status in ["CryptoStaleNeedsRenewal", "CryptoBrokenUntrusted"]:
        return Reject("claim cannot support new credit")

    return Accept
```

关键策略：

```text
旧证明可以作为历史证据；
旧证明不能自动支撑新的信用承兑。
```

------

# 14. 把“算法不稳定性”纳入风险模型

新增风险维度：

```rust
struct CryptoRisk {
    crypto_era_id: CryptoEraId,

    weakest_signature_suite: CryptoSuiteId,
    weakest_hash_suite: HashSuiteId,
    weakest_kem_suite: Option<CryptoSuiteId>,

    years_until_soft_deprecation: Option<i64>,
    years_until_hard_reject: Option<i64>,

    renewal_chain_depth: u32,

    algorithmic_diversity_score: u32,

    has_hash_based_anchor: bool,
    has_code_based_kem_backup: bool,
    has_physical_timestamp_witness: bool,

    risk_label: CryptoRiskLabel,
}

enum CryptoRiskLabel {
    Low,
    Medium,
    High,
    DoNotAcceptForCredit,
    DoNotAcceptAtAll,
}
```

承兑规则：

```python
def can_provisionally_credit(claim, risk_policy):
    crypto_risk = evaluate_crypto_risk(claim)

    if crypto_risk.risk_label in ["DoNotAcceptAtAll", "DoNotAcceptForCredit"]:
        return False

    if crypto_risk.years_until_hard_reject < claim.settlement_horizon_years:
        return False

    if not crypto_risk.has_algorithmic_diversity:
        return risk_policy.allow_single_family_crypto

    return True
```

核心判断：

> 如果一个 claim 的预计 settlement horizon 是 18 年，而其签名算法 10 年后 hard reject，那么不能按正常信用承兑。

------

# 15. 物理辅助证明：Physical Anchor Certificate

为了防止 backdating 和“算法破解后伪造旧签名”，加入：

```rust
struct PhysicalAnchorCertificate {
    anchor_id: Hash,

    subject_hash: Hash,

    observed_at: Vec<TimePositionAttestation>,

    independent_witness_domains: Vec<DomainId>,

    min_witness_distance_ly: f64,

    earliest_publication_coord: SpacetimeCoord,

    latest_possible_forgery_bound: Option<SpacetimeCoord>,

    signature_bundle: SignatureBundle,
}
```

语义：

```text
subject_hash 至少在 earliest_publication_coord 对应的时空区域已经存在。
```

它不能证明内容真实，但能证明：

```text
这个 hash 不是在很久以后才被创造出来的。
```

这对抗的是：

```text
after-break forgery with fake old timestamp
```

------

# 16. 新的核心 invariant

## 16.1 Crypto-era validity

```text
Invariant CryptoEraValidity:

A client may rely on a cryptographic proof P at observer event O only if:

  P's suite is accepted in O's crypto registry
  OR
  P has a renewal chain R such that every renewal occurred before
  the previous suite's hard rejection boundary.
```

## 16.2 No stale proof for new credit

```text
Invariant NoStaleProofForCredit:

If claim.crypto_status in {CryptoStaleNeedsRenewal, CryptoBrokenUntrusted},
then client must not transition claim to ProvisionallyCredited.
```

## 16.3 Algorithmic diversity for archival checkpoint

```text
Invariant ArchivalCheckpointDiversity:

Any checkpoint intended for interstellar settlement must be anchored
by at least two cryptographic families across its renewal chain.

Example:
  lattice-based signature + hash-based signature
```

## 16.4 Physical timestamp sanity

```text
Invariant NoBackdatedCryptoRenewal:

A renewal checkpoint is valid only if its physical anchor event is
lightcone-valid and occurs before the old crypto era hard-reject boundary.
```

------

# 17. 测试系统新增类别

在原来的 ChronoLedger Testbench 里加入：

```text
CryptoEraSimulator
AlgorithmBreakEventGenerator
RenewalChainGenerator
StaleProofInjector
AfterBreakForgeryGenerator
MultiHashCollisionMock
QKDOptionalKeySourceMock
PhysicalAnchorForgeryTest
RadiationFaultInjector
```

## 17.1 算法退役测试

```text
scenario crypto_era_expiry_during_transit {
  crypto_era Era1 {
    sig ML_DSA
    valid_from t=0
    hard_reject_after t=50
  }

  system Earth at (0,0,0) ly
  system FarColony at (80,0,0) ly

  at Earth.t=10:
      create checkpoint C signed under Era1

  message C from Earth to FarColony mode laser

  expect:
      at FarColony.t=90:
          verify(C) == reject
          reject_reason == crypto_era_expired_without_renewal
}
```

## 17.2 续锚测试

```text
scenario renewed_checkpoint_survives_algorithm_deprecation {
  crypto_era Era1 {
    sig ML_DSA
    hard_reject_after t=50
  }

  crypto_era Era2 {
    sig ML_DSA_PLUS_SLH_DSA
    valid_from t=40
  }

  at Earth.t=10:
      create checkpoint C under Era1

  at Earth.t=45:
      create renewal_checkpoint R for C under Era2

  message R from Earth to FarColony mode laser distance 80 ly

  expect:
      at FarColony.t=125:
          verify(C via R) == accept
          crypto_status(C) == CryptoDeprecatedButRenewed
}
```

## 17.3 after-break forgery 测试

```text
scenario attacker_forges_old_checkpoint_after_break {
  crypto_era Era1 {
    sig OLD_SIG
    hard_reject_after t=50
  }

  adversary:
      at t=60:
          forge checkpoint C_fake claiming timestamp t=10

  expect:
      any_client_receiving(C_fake):
          reject_reason in [
              crypto_era_expired_without_renewal,
              missing_physical_anchor,
              impossible_renewal_chain
          ]
}
```

------

# 18. 工程策略：KISS 版本

为了不把系统做成密码动物园，建议分阶段。

## MVP

```text
Signatures:
  ML-DSA for transactions and checkpoints
  SLH-DSA for periodic archival checkpoints

KEM:
  ML-KEM-768 for local sessions
  ML-KEM-1024 for high-value cross-system sessions

Hash:
  SHA-512/384 + SHAKE256 multi-hash for checkpoint roots

No PQ aggregation:
  use Merkleized individual signatures

Crypto era:
  hardcoded registry + explicit deprecation fields

Renewal:
  support RenewalCheckpoint from day one
```

## Research prototype

```text
Add:
  HQC backup KEM once standardized
  physical anchor certificates
  pulsar/VLBI-style timestamp attestations
  split-agent relativistic challenge protocol
  cryptographic algorithm break simulator
  multi-era formal model
```

## Production

```text
Require:
  multi-family signatures for interstellar checkpoints
  mandatory renewal before hard deprecation
  independent archive witnesses
  HSM/MPC key custody
  PQ-safe firmware update path
  radiation-hardened storage
  deterministic reproducible clients
  cryptographic registry governance
```

------

# 19. 最重要的设计结论

最终架构应这样改：

```text
Local BFT ledger
  +
Causal checkpoint mesh
  +
Credit clearing network
  +
Cryptographic survival layer
```

其中密码层的原则是：

```text
1. All long-lived proofs are era-relative.
2. No single algorithm family is trusted forever.
3. Checkpoints must be periodically re-anchored.
4. PQ signatures are mandatory for archival validity.
5. QKD and physical phenomena are optional auxiliaries, not foundations.
6. Lightcone constraints are the strongest physical primitive in the system.
7. Old proofs may prove history; they may not justify new credit.
```

最 KISS 的落地规则可以压缩成一句：

> **星际账本不要问“这个签名是否有效”；要问“这个证明是否在我的光锥内、在当前密码时代内、沿着未断裂的续锚链仍然有效”。**