import"./chunk-native-invoke-gIPiMner.js";import"./index-DjZ15Z0m.js";import{o as e,s as t,t as n}from"./chunk-local-mode-s09XZivk.js";import{_ as r}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{A as i,F as a,L as o,j as s,tt as c,x as l}from"./chunk-_plugin-vue_export-helper-gwY6GXv8.js";import{W as u,ct as d,d as f,n as p,z as m}from"./chunk-http-DtthN4Q-.js";import{C as h,I as g,L as _,c as v,i as y}from"./chunk-gql-client-OVtfbT6_.js";import{t as ee}from"./chunk-websocket-crypto-Ci5-lntu.js";var b=`
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
`,x=`
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
`,te=`
  fragment AppFragment on App {
    clientId
    urlToken
    httpPort
    httpsPort
    appDir
    deviceName
    deviceType
    capabilities
    access
    downloadsDir
    developerMode
    debug
  }
`,w=`
  fragment ChatItemFragment on ChatItem {
    id
    fromId
    toId
    channelId
    createdAt
    content { ... on ChatMessageContent { __typename text format linkPreviews { url title description image imageWidth imageHeight siteName domain createdAt } items { type attachment { id uri size durationMs width height summary fileName } } } ... on ChatShareContent { __typename shareId urlToken peerInfo { id ip port } name itemCount totalSize expiresAt } }
    status
    deliveryResults { peerId peerName error }
  }
`,T=`
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
    photo
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
    thumbHash
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
    thumbHash
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
`,ne=`
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
`,F=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,I=`
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
`,re=`
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
`,ie=`
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
`,L=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,R=`
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
`,z=`
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
`,B=`
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
`,V=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,H=`
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

`,U=function(e){return e.InvalidSubscription=`INVALID_SUBSCRIPTION`,e.SubscriptionResultTooLarge=`SUBSCRIPTION_RESULT_TOO_LARGE`,e.ResyncRequired=`RESYNC_REQUIRED`,e.SlowClient=`SLOW_CLIENT`,e}({}),W=new Set(Object.values(U));function G(e){return typeof e==`string`&&W.has(e)?e:void 0}function K(e,t){return q(e,t)}function q(n,r){let i=m(`/graphql`,r),a=new URL(i,f()),o=e(),s=u();if(!s)throw Error(`Subscription session missing`);a.searchParams.set(`cid`,o);let c=p(a.toString(),t());c.binaryType=`arraybuffer`;let l=!0,d=!1,g,_,y,b=setTimeout(()=>S(Error(`Subscription handshake timeout`)),1e4),x={send(e){try{if(!l||!d||!g)throw Error(`Subscription connection unavailable`);i!==`/graphql`&&J(e)&&(e={...e,payload:{...e.payload,query:v(e.payload.query)}}),c.send(g.encode(new TextEncoder().encode(JSON.stringify(e))))}catch(e){S(e)}},close(){l=!1,g=void 0,clearTimeout(b),clearTimeout(y),clearInterval(_),c.close()}};function S(e){l&&(x.close(),n.dropped(e))}return c.onopen=()=>{if(l)try{g=new ee(o,h(s)),c.send(g.registration)}catch(e){S(e)}},c.onmessage=e=>{if(l)try{if(!g||!(e.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(d){let t=JSON.parse(new TextDecoder().decode(g.decode(new Uint8Array(e.data))));typeof t==`object`&&t&&`type`in t&&t.type===`pong`?clearTimeout(y):n.message(t)}else{if(g.acknowledge(new Uint8Array(e.data)),c.send(g.encode(new Uint8Array)),d=!0,clearTimeout(b),!l||(n.opened(),!l))return;_=setInterval(()=>{x.send({type:`ping`}),l&&(y=setTimeout(()=>S(Error(`Subscription heartbeat timeout`)),1e4))},2e4)}}catch(e){S(e)}},c.onerror=()=>S(Error(`Subscription connection error`)),c.onclose=()=>S(Error(`Subscription connection closed`)),x}function J(e){if(!e||typeof e!=`object`||!(`type`in e)||e.type!==`subscribe`||!(`payload`in e))return!1;let t=e.payload;return!!t&&typeof t==`object`&&`query`in t&&typeof t.query==`string`}var Y=class{openConnection;constructor(e=K){this.openConnection=e}connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;attaching=!1;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;this.attaching=!0;let e=++this.generation;try{this.connection=this.openConnection({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&(!this.attaching&&!this.pending.size&&(this.attaching=!0,this.pending=new Set(this.ids.keys())),this.settled(t.id)),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>G(e.extensions?.code)===U.ResyncRequired)){this.ids.delete(t.id),this.pending.delete(t.id),this.attaching=!0,this.send(n);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.attaching&&(this.attaching=!1,this.ready())}dropped(e){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&!n()&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1,this.attaching=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},X=new Y,ae={audios:`audioCount(query: $mediaQuery)`,images:`imageCount(query: $mediaQuery)`,videos:`videoCount(query: $mediaQuery)`,docs:`docCount(query: "")`,packages:`packageCount(query: "")`,notes:`noteCount(query: "")`,feedEntries:`feedEntryCount(query: "")`,messages:`smsCount(query: "")`,calls:`callCount(query: "")`,contacts:`contactCount(query: "")`};function oe(e=[]){let t=e.map(e=>ae[e]).join(`
    `);return`
  query homeStats${t.includes(`$mediaQuery`)?`($mediaQuery: String!)`:``} {
    ${t}
    mounts {
      ...MountFragment
    }
  }
  ${b}
`}function Z(e){return e instanceof d?e.status===403?_.RemoteAccessDisabled:e.message:g.NetworkError}function Q(e){if(e)return typeof e==`function`?e():e}function se(e){let t=r(!1),n=r(),u=0;async function f(r,i=!1){if(e.enabled&&!e.enabled())return;let a=u;t.value=!0;try{let t=r??Q(e.variables),o=await y(typeof e.document==`function`?e.document():e.document,t,{fresh:i});if(a!==u)return;o.errors?.length?e.handle(o.data,o.errors[0].message):(n.value=o.data,e.handle(o.data,``))}catch(t){if(a!==u||t instanceof d&&t.canceled)return;e.handle(void 0,Z(t))}finally{a===u&&(t.value=!1)}}f();let p=!0,m=l();return m&&(a(()=>{p=!1}),s(()=>{p=!0})),typeof e.variables==`function`&&c(e.variables,async()=>{await i(),p&&f()},{deep:!0}),typeof e.document==`function`&&c(e.document,async()=>{await i(),p&&f()}),e.enabled&&c(e.enabled,async(e,t)=>{await i(),p&&e&&!t&&f()}),m&&o(X.onAttached(()=>{p&&(u++,f(void 0,!0))})),{loading:t,result:n,refetch:f}}function ce(e){let t=r(!1),n=r(),i=0,c=0,u=0,f,p=0,m=!0,h;function g(){p++,u=0,f=void 0,t.value=!1}if(l()){s(()=>{m=!0}),a(()=>{m=!1,g()});let e=X.onAttached(()=>{m&&h&&_(h.variables,{...h.options,latest:!0,force:!0})});o(()=>{g(),e()})}async function _(r,a={}){a.force&&g();let o=p;h={variables:r,options:a};let s=++i,l=a.latest?++c:void 0,m=r??Q(e.variables),_=m?{...m}:void 0,v={variables:_,requestId:s,meta:a.meta};a.latest?f=s:u++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=a.force?await y(t,_,{fresh:!0}):await y(t,_);if(o!==p||a.latest&&l!==c)return;r.errors?.length?e.handle(r.data,r.errors[0].message,v):(n.value=r.data,e.handle(r.data,``,v))}catch(t){if(o!==p||t instanceof d&&t.canceled)return;(!a.latest||l===c)&&e.handle(void 0,Z(t),v)}finally{o===p&&(a.latest?f===s&&(f=void 0):u--,t.value=u>0||f!==void 0)}}return{loading:t,result:n,fetch:_}}var le=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${w}
`;`${w}`;var ue=`
  query {
    peers {
      id
      name
      ip
      status
      online
      managed
      port
      deviceType
      createdAt
      updatedAt
    }
  }
`,de=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${w}
`,fe=`
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
`,pe=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,me=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${H}
`,he=`
  query ($path: String!) {
    fileInfo(path: $path) {
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
`,ge=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${T}
`,_e=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,ve=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${E}
`,ye=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${D}
`,be=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${O}
`,xe=`
  query {
    contactSources {
      name
      type
    }
  }
`,Se=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${k}
`,Ce=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: ImageSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${j}
`,we=`
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
`,Te=`
  query($volumeId: ID) {
    fileIndexScanProgress(volumeId: $volumeId) { volumeId
      indexed
      pending
      total
      state
      error
    }
  }
`,Ee=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: VideoSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${M}
`,De=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: AudioSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${N}
`,Oe=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${A}
`,ke=`
  query directoryEntries($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      path
      isDir
      isSystem
    }
  }
`,Ae=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${A}
`,je=`
  query {
    mounts {
      ...MountFragment
    }
  }
  ${b}
`,Me=`
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
`,Ne=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,Pe=`
  query {
    app {
      ...AppFragment
    }
  }
  ${te}
`,Fe=`
  query mediaPlayHistory($query: String!, $offset: Int!, $limit: Int!) {
    mediaPlayHistory(query: $query, offset: $offset, limit: $limit) {
      positionMs
    }
  }
`,Ie=`
  query audioQueue($offset: Int!, $limit: Int!) {
    items: audioQueueItems(query: "", offset: $offset, limit: $limit) {
      ...AudioItemFragment
    }
    total: audioQueueItemCount(query: "")
    playback: audioPlayback {
      mediaId
      path
    }
    prefs(keys: ["audio_play_mode"])
  }
  ${C}
`,Le=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${x}
`,Re=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      coverItems { path updatedAt thumbHash }
    }
  }
`,ze=`
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
`,Be=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${P}
`,Ve=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${F}
`,He=`
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
`,Ue=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${F}
  ${x}
`,We=`
  query bucketsTags($type: MediaDataType!, $tagType: DataType!) {
    tags(type: $tagType) {
      ...TagFragment
    }
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      coverItems { path updatedAt thumbHash }
    }
  }
  ${x}
`,Ge=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${F}
  ${I}
`,Ke=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,qe=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Je=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: DocSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${ne}
`,Ye=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,Xe=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Ze=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Qe=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      feedId
      count
    }
  }
`,$e=`
  query {
    total: contactCount(query: "")
  }
`,et=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:INCOMING")
    outgoing: callCount(query: "type:OUTGOING")
    missed: callCount(query: "type:MISSED")
  }
`,tt=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,nt=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${E}
`,rt=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${D}
`,it=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,at=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${re}
`,ot=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,st=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,ct=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,lt=`
  query {
    screenMirrorControlEnabled
  }
`,ut=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,dt=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${ie}
`,ft=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${L}
`,pt=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${R}
  ${z}
`,mt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ht=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,gt=`
  query {
    appLogPath
  }
`,_t=`
  query {
    dbPath
  }
`,vt=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId) { index sizeBytes }
  }
`,yt=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,$=`
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
`,bt=`
  query {
    prefs
  }
`,xt=`
  query {
    dbTables {
      name
      createSql
      columns {
        name
        dataType
        notNull
        defaultValue
        primaryKey
      }
    }
  }
`,St=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,Ct=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,wt=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${B}
  ${V}
`,Tt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,Et=`
  query NearbyDevices {
    nearbyDevices { signaturePublicKey id name ips bestIp port deviceType version platform lastSeen status discoveryMethods remoteAccess }
    isDiscovering
  }
`,Dt=`
  query indexedFiles($query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    files: indexedFiles(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) { ...FileFragment }
    count: indexedFileCount(query: $query)
  }
  ${A}
`,Ot=`
  subscription fileSearchResults($root: String!, $query: String!, $offset: Int!, $limit: Int!, $sortBy: FileSortBy!) {
    fileSearchResults(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      items { ...FileFragment }
      count
      offset
      complete
    }
  }
  ${A}
`,kt=`
  query exportFeeds {
    exportFeeds
  }
`,At=`
  query exportNotes($query: String!) {
    exportNotes(query: $query)
  }
`,jt=`
  query trashedFiles($offset: Int!, $limit: Int!) {
    trashedFiles(query: "", offset: $offset, limit: $limit) {
      name path size updatedAt isDir childCount
    }
  }
`;export{yt as $,Ye as A,K as At,Te as B,x as Bt,St as C,jt as Ct,mt as D,oe as Dt,pt as E,Ee as Et,He as F,O as Ft,we as G,Ot as H,Qe as I,I as It,ce as J,Ce as K,Ge as L,F as Lt,kt as M,V as Mt,At as N,H as Nt,ke as O,Y as Ot,Ne as P,w as Pt,Fe as Q,Ve as R,A as Rt,_t as S,Le as St,xt as T,Xe as Tt,Oe as U,he as V,Ke as W,st as X,se as Y,Re as Z,le as _,ve as _t,Tt as a,dt as at,xe as b,ge as bt,qe as c,at as ct,wt as d,bt as dt,je as et,We as f,Ae as ft,de as g,_e as gt,me as h,ct as ht,ht as i,ze as it,Je as j,B as jt,Me as k,X as kt,Ie as l,ue as lt,Se as m,lt as mt,Pe as n,it as nt,nt as o,Ze as ot,et as p,ut as pt,Dt as q,gt as r,Be as rt,rt as s,ot as st,fe as t,Et as tt,De as u,$ as ut,ft as v,ye as vt,Ct as w,vt as wt,be as x,pe as xt,$e as y,tt as yt,Ue as z,P as zt};