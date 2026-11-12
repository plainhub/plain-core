import"./chunk-core-iC9FAxFL.js";import{a as e,o as t}from"./chunk-client-id-sMazQ4of.js";import{m as n}from"./chunk-login-peers-xdaIJaIr.js";import{_ as r}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{$ as i,A as a,F as o,L as s,j as c,x as l}from"./chunk-_plugin-vue_export-helper-C_erUPa1.js";import{H as u,W as d,f,n as p}from"./chunk-http-DVRN_e2U.js";import{a as m,t as h,x as g}from"./chunk-gql-client-Czt4ls5u.js";import{t as _}from"./chunk-websocket-crypto-8hDZs1lX.js";function v(e,t){return y(e,t)}function y(n,r){let i=new URL(r?u(r,`graphql`):`/graphql`,f()),a=e(),o=d();if(!o)throw Error(`Subscription session missing`);i.searchParams.set(`cid`,a);let s=p(i.toString(),t());s.binaryType=`arraybuffer`;let c=!0,l=!1,m,h,v,y=setTimeout(()=>x(Error(`Subscription handshake timeout`)),1e4),b={send(e){try{if(!c||!l||!m)throw Error(`Subscription connection unavailable`);s.send(m.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){x(e)}},close(){c=!1,m=void 0,clearTimeout(y),clearTimeout(v),clearInterval(h),s.close()}};function x(e){c&&(b.close(),n.dropped(e))}return s.onopen=()=>{if(c)try{m=new _(a,g(o)),s.send(m.registration)}catch(e){x(e)}},s.onmessage=e=>{if(c)try{if(!m||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(l){let t=JSON.parse(new TextDecoder().decode(m.decode(new Uint8Array(e.data))));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(v):n.message(t)}else{if(m.acknowledge(new Uint8Array(e.data)),s.send(m.encode(new Uint8Array)),l=!0,clearTimeout(y),!c||(n.opened(),!c))return;h=setInterval(()=>{b.send({type:`ping`}),c&&(v=setTimeout(()=>x(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){x(e)}},s.onerror=()=>x(Error(`Subscription connection error`)),s.onclose=()=>x(Error(`Subscription connection closed`)),b}var b=new class{openConnection;constructor(e=v){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>e.message===`resync_required`)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!n()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},x=`
  fragment MountFragment on Mount {
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
`,S=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,C=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,w=`
  fragment AudioItemFragment on AudioItem {
    mediaId
    title
    artist
    path
    durationMs
  }
`,T=`
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
`,E=`
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
  ${C}
`,D=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,O=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,k=`
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
  ${C}
`,A=`
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
  ${C}
`,j=`
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
`,M=`
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
  ${C}
`,N=`
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
  ${C}
`,P=`
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
  ${C}
`,F=`
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
  ${C}
`,I=`
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
  ${C}
`,L=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,R=`
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
  ${C}
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
`,z=`
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
`,B=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,V=`
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
`,H=`
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
`,U=`
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
`,W=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,G=`
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

`,K={audios:`audioCount(query: $mediaQuery)`,images:`imageCount(query: $mediaQuery)`,videos:`videoCount(query: $mediaQuery)`,docs:`docCount(query: "")`,packages:`packageCount(query: "")`,notes:`noteCount(query: "")`,feedEntries:`feedEntryCount(query: "")`,messages:`smsCount(query: "")`,calls:`callCount(query: "")`,contacts:`contactCount(query: "")`};function q(e=[]){let t=e.map(e=>K[e]).join(`
    `);return`
  query homeStats${t.includes(`$mediaQuery`)?`($mediaQuery: String!)`:``} {
    ${t}
    mounts {
      ...MountFragment
    }
  }
  ${x}
`}function J(e){return e instanceof h?e.status===403?`remote_access_disabled`:e.message:`network_error`}function Y(e){if(e)return typeof e==`function`?e():e}function X(e){let t=r(!1),n=r(),u=0;async function d(r,i=!1){if(e.enabled&&!e.enabled())return;let a=u;t.value=!0;try{let t=r??Y(e.variables),o=await m(typeof e.document==`function`?e.document():e.document,t,{fresh:i});if(a!==u)return;o.errors?.length?e.handle(o.data,o.errors[0].message):(n.value=o.data,e.handle(o.data,``))}catch(t){if(a!==u||t instanceof h&&t.cancelled)return;e.handle(void 0,J(t))}finally{a===u&&(t.value=!1)}}d();let f=!0,p=l();return p&&(o(()=>{f=!1}),c(()=>{f=!0})),typeof e.variables==`function`&&i(e.variables,async()=>{await a(),f&&d()},{deep:!0}),typeof e.document==`function`&&i(e.document,async()=>{await a(),f&&d()}),e.enabled&&i(e.enabled,async(e,t)=>{await a(),f&&e&&!t&&d()}),p&&s(b.onAttached(()=>{f&&(u++,d(void 0,!0))})),{loading:t,result:n,refetch:d}}function Z(e){let t=r(!1),n=r(),i=0,a=0,u=0,d,f=0,p=!0,g;l()&&(c(()=>{p=!0}),o(()=>{p=!1}),s(b.onAttached(()=>{p&&g&&_(g.variables,{...g.options,latest:!0,force:!0})})));async function _(r,o={}){let s=o.force?++f:f;g={variables:r,options:o};let c=++i,l=o.latest?++a:void 0,p=r??Y(e.variables),_=p?{...p}:void 0,v={variables:_,requestId:c,meta:o.meta};o.latest?d=c:u++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=o.force?await m(t,_,{fresh:!0}):await m(t,_);if(s!==f||o.latest&&l!==a)return;r.errors?.length?e.handle(r.data,r.errors[0].message,v):(n.value=r.data,e.handle(r.data,``,v))}catch(t){if(s!==f||t instanceof h&&t.cancelled)return;(!o.latest||l===a)&&e.handle(void 0,J(t),v)}finally{o.latest?d===c&&(d=void 0):u--,t.value=u>0||d!==void 0}}return{loading:t,result:n,fetch:_}}var Q=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${E}
`;`${E}`;var ne=`
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
`,re=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${E}
`,ie=`
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
`,ae=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,oe=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${G}
`,se=`
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
`,ce=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${ee}
`,le=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,ue=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${D}
`,de=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${O}
`,fe=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${k}
`,pe=`
  query {
    contactSources {
      name
      type
    }
  }
`,me=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${A}
`,he=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${M}
`,ge=`
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
`,_e=`
  query {
    scanProgress {
      indexed
      pending
      total
      state
      error
    }
  }
`,ve=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${N}
`,ye=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${P}
`,be=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${j}
`,xe=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,Se=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${j}
`,Ce=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${x}
`,we=`
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
`,Te=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,Ee=`
  query {
    app {
      ...AppFragment
    }
  }
  ${T}
`,De=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Oe=`
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
  ${w}
`,ke=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${S}
`,Ae=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,je=`
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
  ${C}
`,Me=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${F}
`,Ne=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${L}
`,Pe=`
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
  ${C}
`,Fe=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${L}
  ${S}
`,Ie=`
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
  ${S}
`,Le=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${L}
  ${R}
`,Re=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,ze=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Be=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${I}
`,Ve=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,He=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Ue=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,We=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      id
      count
    }
  }
`,Ge=`
  query {
    total: contactCount(query: "")
  }
`,Ke=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,qe=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,Je=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${D}
`,Ye=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${O}
`,Xe=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,Ze=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${te}
`,Qe=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,$e=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,et=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,tt=`
  query {
    screenMirrorControlEnabled
  }
`,nt=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,rt=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${z}
`,it=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${B}
`,at=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${V}
  ${H}
`,ot=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,st=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,ct=`
  query {
    appLogPath
  }
`,lt=`
  query {
    dbPath
  }
`,ut=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId)
  }
`,dt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,ft=`
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
`,$=`
  query {
    prefs
  }
`,pt=`
  query {
    dbTables
  }
`,mt=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,ht=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,gt=`
  query DbTableInfo($table: String!) {
    dbTableInfo(table: $table) {
      createSql
      idKey
    }
  }
`,_t=`
  query DbTableColumns($table: String!) {
    dbTableColumns(table: $table) {
      name
      dataType
      notNull
      defaultValue
      primaryKey
    }
  }
`,vt=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${U}
  ${W}
`,yt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,bt=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    pairingStates { deviceId generation mode phase error verificationCode identityChanged }
    mdnsState { scanning }
  }
`,xt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${j}
`,St=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${j}
`;export{Ce as $,xe as A,G as At,se as B,v as Bt,_t as C,ut as Ct,pt as D,w as Dt,ht as E,q as Et,Pe as F,L as Ft,he as G,be as H,We as I,j as It,X as J,xt as K,Le as L,F as Lt,Ve as M,B as Mt,Be as N,k as Nt,at as O,U as Ot,Te as P,R as Pt,dt as Q,Ne as R,S as Rt,lt as S,ke as St,mt as T,ve as Tt,Re as U,St as V,ge as W,Ae as X,$e as Y,De as Z,Q as _,ue as _t,yt as a,Ue as at,pe as b,ce as bt,ze as c,ne as ct,vt as d,Se as dt,bt as et,Ie as f,nt as ft,re as g,le as gt,oe as h,et as ht,st as i,rt as it,we as j,E as jt,ot as k,W as kt,Oe as l,ft as lt,me as m,tt as mt,Ee as n,Me as nt,Je as o,Qe as ot,Ke as p,_e as pt,Z as q,ct as r,je as rt,Ye as s,Ze as st,ie as t,Xe as tt,ye as u,$ as ut,it as v,de as vt,gt as w,He as wt,fe as x,ae as xt,Ge as y,qe as yt,Fe as z,b as zt};