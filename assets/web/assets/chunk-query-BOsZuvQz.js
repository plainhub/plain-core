import"./chunk-core-iC9FAxFL.js";import{a as e,o as t}from"./chunk-client-id-sMazQ4of.js";import{m as n}from"./chunk-login-peers-xdaIJaIr.js";import{_ as r}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{$ as i,A as a,F as o,L as s,j as c,x as l}from"./chunk-_plugin-vue_export-helper-C_erUPa1.js";import{K as u,W as d,i as f,m as p}from"./chunk-time-sync-XkXcOZTR.js";import{a as m,t as h,x as g}from"./chunk-gql-client-DZqRt730.js";import{t as _}from"./chunk-graphql-subscription-crypto-D8KBo-42.js";function v(e,t){return y(e,t)}function y(n,r){let i=new URL(r?d(r,`graphql`):`/graphql`,p()),a=e(),o=u();if(!o)throw Error(`Subscription session missing`);i.searchParams.set(`cid`,a);let s=f(i.toString(),t(),`plain-graphql-ws-v1`);s.binaryType=`arraybuffer`;let c=!0,l=!1,m,h,v,y=setTimeout(()=>x(Error(`Subscription handshake timeout`)),1e4),b={send(e){try{if(!c||!l||!m)throw Error(`Subscription connection unavailable`);s.send(m.encode(e))}catch(e){x(e)}},close(){c=!1,m=void 0,clearTimeout(y),clearTimeout(v),clearInterval(h),s.close()}};function x(e){c&&(b.close(),n.dropped(e))}return s.onopen=()=>{if(c){if(s.protocol!==`plain-graphql-ws-v1`){x(Error(`Subscription protocol mismatch`));return}try{m=new _(a,g(o)),s.send(m.registration)}catch(e){x(e)}}},s.onmessage=e=>{if(c)try{if(!m||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(!l)m.acknowledge(new Uint8Array(e.data)),l=!0,clearTimeout(y),n.opened(),h=setInterval(()=>{b.send({type:`ping`}),v=setTimeout(()=>x(Error(`Subscription heartbeat timeout`)),1e4)},2e4);else{let t=m.decode(new Uint8Array(e.data));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(v):n.message(t)}}catch(e){x(e)}},s.onerror=()=>x(Error(`Subscription connection error`)),s.onclose=()=>x(Error(`Subscription connection closed`)),b}var b=new class{openConnection;constructor(e=v){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>e.message===`resync_required`)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!n()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},x=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,S=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,C=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,w=`
  fragment AppFragment on App {
    clientId
    urlToken
    httpPort
    httpsPort
    appDir
    deviceName
    deviceType
    capabilities
    buildChannel
    access
    downloadsDir
    developerMode
    debug
  }
`,T=`
  fragment ChatItemFragment on ChatItem {
    id
    fromId
    toId
    channelId
    createdAt
    content
    status
    statusData
  }
`,ee=`
  fragment SmsFragment on Sms {
    id
    body
    address
    serviceCenter
    sentAt
    type
    threadId
    subscriptionId
    isMms
    attachments {
      path
      contentType
      name
    }
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,E=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,D=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,O=`
  fragment ContactFragment on Contact {
    id
    suffix
    prefix
    firstName
    middleName
    lastName
    updatedAt
    notes
    source
    thumbnailId
    starred
    phoneNumbers {
      value
      type
      label
      normalizedNumber
    }
    addresses {
      value
      type
      label
    }
    emails {
      value
      type
      label
    }
    websites {
      value
      type
      label
    }
    events {
      value
      type
      label
    }
    ims {
      value
      protocol
      customProtocol
    }
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,k=`
  fragment CallFragment on Call {
    id
    name
    number
    durationSec
    accountId
    startedAt
    photoId
    type
    geo {
      country
      numberType
      carrier
      description
    }
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,A=`
  fragment FileFragment on File {
    isSystem
    path
    isDir
    createdAt
    updatedAt
    size
    childCount
    mediaId
  }
`,j=`
  fragment ImageFragment on Image {
    id
    title
    path
    size
    bucketId
    takenAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,M=`
  fragment VideoFragment on Video {
    id
    title
    path
    durationMs
    size
    bucketId
    createdAt
    updatedAt
    takenAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,N=`
  fragment AudioFragment on Audio {
    id
    title
    artist
    path
    durationMs
    size
    bucketId
    albumFileId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,P=`
  fragment NoteFragment on Note {
    id
    title
    content
    deletedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,F=`
  fragment DocFragment on Doc {
    id
    title
    path
    extension
    size
    bucketId
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,I=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,L=`
  fragment FeedEntryFragment on FeedEntry {
    id
    title
    url
    image
    author
    description
    content
    feedId
    rawId
    publishedAt
    createdAt
    updatedAt
    tags {
      ...TagSubFragment
    }
  }
  ${S}
`,te=`
  fragment PackageFragment on Package {
    id
    name
    type
    version
    path
    size
    certs {
      issuer
      subject
      serialNumber
      validFrom
      validTo
    }
    installedAt
    updatedAt
  }
`,R=`
  fragment NotificationFragment on Notification {
    id
    onlyOnce
    isClearable
    appId
    appName
    postedAt
    silent
    title
    body
    actions
    replyActions
  }
`,z=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,B=`
  fragment DeviceInfoFragment on DeviceInfo {
    name
    platform
    manufacturer
    model
    osName
    osVersion
    kernelVersion
    appVersion
    appBuildNumber
    language
    cpuArch
    cpuModel
    totalMemory
    totalStorage
    display {
      width
      height
      density
    }
    android {
      sdkVersion
      versionCodeName
      securityPatch
      bootloader
      fingerprint
      hardware
      radioVersion
      board
      buildBrand
      buildNumber
      device
      javaVmVersion
      glEsVersion
      buildTime
    }
  }
`,V=`
  fragment DeviceStatusFragment on DeviceStatus {
    uptimeSec
    batteryLevel
    charging
    temperatures {
      label
      celsius
    }
    cpuUsage
    memoryAvailable
    storageAvailable
  }
`,H=`
  fragment BookmarkFragment on Bookmark {
    id
    url
    title
    faviconPath
    groupId
    pinned
    clickCount
    lastClickedAt
    sortOrder
    createdAt
    updatedAt
  }
`,U=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,W=`
  fragment ChatChannelFragment on ChatChannel {
    actions { isOwner canRename canManageMembers canLeave canDelete }
    id
    name
    ownerId
    members {
      ...ChatChannelMemberFragment
    }
    version
    status
    createdAt
    updatedAt
  }

  fragment ChatChannelMemberFragment on ChatChannelMember {
    peerId
    status
  }

`;function G(e){return e instanceof h?e.status===403?`remote_access_disabled`:e.message:`network_error`}function K(e){if(e)return typeof e==`function`?e():e}function q(e){let t=r(!1),n=r(),u=0;async function d(r,i=!1){if(e.enabled&&!e.enabled())return;let a=u;t.value=!0;try{let t=r??K(e.variables),o=await m(typeof e.document==`function`?e.document():e.document,t,{fresh:i});if(a!==u)return;o.errors?.length?e.handle(o.data,o.errors[0].message):(n.value=o.data,e.handle(o.data,``))}catch(t){if(a!==u||t instanceof h&&t.cancelled)return;e.handle(void 0,G(t))}finally{a===u&&(t.value=!1)}}d();let f=!0,p=l();return p&&(o(()=>{f=!1}),c(()=>{f=!0})),typeof e.variables==`function`&&i(e.variables,async()=>{await a(),f&&d()},{deep:!0}),typeof e.document==`function`&&i(e.document,async()=>{await a(),f&&d()}),e.enabled&&i(e.enabled,async(e,t)=>{await a(),f&&e&&!t&&d()}),p&&s(b.onAttached(()=>{f&&(u++,d(void 0,!0))})),{loading:t,result:n,refetch:d}}function J(e){let t=r(!1),n=r(),i=0,a=0,u=0,d,f=0,p=!0,g;l()&&(c(()=>{p=!0}),o(()=>{p=!1}),s(b.onAttached(()=>{p&&g&&_(g.variables,{...g.options,latest:!0,force:!0})})));async function _(r,o={}){let s=o.force?++f:f;g={variables:r,options:o};let c=++i,l=o.latest?++a:void 0,p=r??K(e.variables),_=p?{...p}:void 0,v={variables:_,requestId:c,meta:o.meta};o.latest?d=c:u++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=o.force?await m(t,_,{fresh:!0}):await m(t,_);if(s!==f||o.latest&&l!==a)return;r.errors?.length?e.handle(r.data,r.errors[0].message,v):(n.value=r.data,e.handle(r.data,``,v))}catch(t){if(s!==f||t instanceof h&&t.cancelled)return;(!o.latest||l===a)&&e.handle(void 0,G(t),v)}finally{o.latest?d===c&&(d=void 0):u--,t.value=u>0||d!==void 0}}return{loading:t,result:n,fetch:_}}var Y=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${T}
`;`${T}`;var X=`
  query {
    peers {
      id
      name
      ip
      status
      online
      port
      deviceType
      createdAt
      updatedAt
    }
  }
`,Z=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${T}
`,Q=`
  query appFiles($offset: Int!, $limit: Int!) {
    appFiles(query: "", offset: $offset, limit: $limit) {
      id
      size
      mimeType
      fileName
      createdAt
      updatedAt
    }
    appFileCount(query: "")
  }
`,ne=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,re=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${W}
`,ie=`
  query ($path: String!, $fileName: String) {
    fileInfo(path: $path, fileName: $fileName) {
      ... on FileInfo {
        path
        updatedAt
        size
      }
      data {
        ... on ImageFileInfo {
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on VideoFileInfo {
          durationMs
          width
          height
          location {
            latitude
            longitude
          }
        }
        ... on AudioFileInfo {
          durationMs
          location {
            latitude
            longitude
          }
        }
      }
    }
  }
`,ae=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ee}
`,oe=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,se=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${E}
`,ce=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${D}
`,le=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${O}
`,ue=`
  query {
    contactSources {
      name
      type
    }
  }
`,de=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${k}
`,fe=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${j}
`,pe=`
  query {
    imageSearchStatus {
      status
      downloadProgress
      errorMessage
      modelSize
      modelDir
      isIndexing
      totalImages
      indexedImages
    }
  }
`,me=`
  query {
    scanProgress {
      indexed
      pending
      total
      state
      error
    }
  }
`,he=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${M}
`,ge=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${N}
`,_e=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${A}
`,ve=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,ye=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${A}
`,be=`
  query {
    mounts {
      id
      name
      path
      mountPoint
      fsType
      totalBytes
      usedBytes
      freeBytes
      remote
      alias
      driveType
      diskId
      label
      diskModel
    }
  }
`,xe=`
  query {
    disks {
      id
      name
      path
      sizeBytes
      removable
      model
    }
  }
`,Se=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,Ce=`
  query {
    app {
      ...AppFragment
    }
  }
  ${w}
`,we=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Te=`
  query audioQueue($offset: Int!, $limit: Int!) {
    items: audioQueueItems(query: "", offset: $offset, limit: $limit) {
      ...AudioItemFragment
    }
    total: audioQueueItemCount
    playback: audioPlayback {
      revision
      mode
      mediaId
      path
      isPlaying
      positionMs
    }
  }
  ${C}
`,Ee=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${x}
`,De=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,Oe=`
  query notes($offset: Int!, $limit: Int!, $query: String!) {
    notes(query: $query, offset: $offset, limit: $limit) {
      id
      title
      deletedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    noteCount(query: $query)
  }
  ${S}
`,ke=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${P}
`,Ae=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${I}
`,je=`
  query feedEntries($offset: Int!, $limit: Int!, $query: String!) {
    items: feedEntries(query: $query, offset: $offset, limit: $limit) {
      id
      title
      url
      image
      author
      feedId
      rawId
      publishedAt
      createdAt
      updatedAt
      tags {
        ...TagSubFragment
      }
    }
    total: feedEntryCount(query: $query)
  }
  ${S}
`,Me=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${I}
  ${x}
`,Ne=`
  query bucketsTags($type: MediaDataType!, $tagType: DataType!) {
    tags(type: $tagType) {
      ...TagFragment
    }
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
  ${x}
`,Pe=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${I}
  ${L}
`,Fe=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,Ie=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Le=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${F}
`,Re=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,ze=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Be=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Ve=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      id
      count
    }
  }
`,He=`
  query {
    total: contactCount(query: "")
  }
`,Ue=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,We=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,Ge=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${E}
`,Ke=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${D}
`,qe=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,Je=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${te}
`,Ye=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,Xe=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,Ze=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,Qe=`
  query {
    screenMirrorControlEnabled
  }
`,$e=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,et=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${R}
`,tt=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${z}
`,nt=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${B}
  ${V}
`,rt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,it=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,at=`
  query {
    appLogPath
  }
`,ot=`
  query {
    dbPath
  }
`,st=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId)
  }
`,ct=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,lt=`
  query {
    pomodoroToday {
      date
      completedCount
      currentRound
      timeLeftSec
      totalTimeSec
      isRunning
      isPaused
      state
    }
    pomodoroSettings {
      workDurationMin
      shortBreakDurationMin
      longBreakDurationMin
      pomodorosBeforeLongBreak
      showNotification
      playSoundOnComplete
    }
  }
`,ut=`
  query {
    prefs
  }
`,dt=`
  query {
    dbTables
  }
`,ft=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,$=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,pt=`
  query DbTableInfo($table: String!) {
    dbTableInfo(table: $table) {
      createSql
      idKey
    }
  }
`,mt=`
  query DbTableColumns($table: String!) {
    dbTableColumns(table: $table) {
      name
      dataType
      notNull
      defaultValue
      primaryKey
    }
  }
`,ht=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${H}
  ${U}
`,gt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,_t=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    pairingStates { deviceId generation mode phase error verificationCode identityChanged }
    mdnsState { scanning }
  }
`,vt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${A}
`,yt=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${A}
`;export{be as $,ve as A,T as At,ie as B,mt as C,st as Ct,dt as D,H as Dt,$ as E,C as Et,je as F,A as Ft,fe as G,_e as H,Ve as I,P as It,q as J,vt as K,Pe as L,x as Lt,Re as M,O as Mt,Le as N,L as Nt,nt as O,U as Ot,Se as P,I as Pt,ct as Q,Ae as R,b as Rt,ot as S,Ee as St,ft as T,he as Tt,Fe as U,yt as V,pe as W,De as X,Xe as Y,we as Z,Y as _,se as _t,gt as a,Be as at,ue as b,ae as bt,Ie as c,X as ct,ht as d,ye as dt,_t as et,Ne as f,$e as ft,Z as g,oe as gt,re as h,Ze as ht,it as i,et as it,xe as j,z as jt,rt as k,W as kt,Te as l,lt,de as m,Qe as mt,Ce as n,ke as nt,Ge as o,Ye as ot,Ue as p,me as pt,J as q,at as r,Oe as rt,Ke as s,Je as st,Q as t,qe as tt,ge as u,ut,tt as v,ce as vt,pt as w,ze as wt,le as x,ne as xt,He as y,We as yt,Me as z,v as zt};