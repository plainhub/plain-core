import{a as e}from"./chunk-core-Cig7zpmj.js";import{B as t,Ct as n,H as r,Ot as i,V as a,a as o,c as s,d as c,dt as l,kt as u,rt as d,t as f,w as p}from"./chunk-gql-client-Ba31yB1u.js";import{I as m,M as h,R as g,et as _,j as v,ut as y,x as b}from"./chunk-_plugin-vue_export-helper-DhhQ8qWg.js";var x=new TextEncoder;function S(e){let t=new Uint8Array(e.reduce((e,t)=>e+4+t.length,0)),n=0;for(let r of e)new DataView(t.buffer).setUint32(n,r.length,!1),t.set(r,n+4),n+=4+r.length;return t}function C(e,n,r){let i=t(24),o=a(e,i,r).encrypt(n),s=new Uint8Array(24+o.length);return s.set(i),s.set(o,24),s}function w(e,t,n){return a(e,t.subarray(0,24),n).decrypt(t.subarray(24))}var ee=class{client;key;registration;binding;sent=0n;received=0n;constructor(e,n){this.client=e,this.key=n;let r=[...t(32)].map(e=>e.toString(16).padStart(2,`0`)).join(``),i=C(n,x.encode(JSON.stringify({timestampMillis:s(),clientNonce:r})),S([x.encode(`plain-graphql-init-v1`),x.encode(`/graphql`),x.encode(e)]));this.registration=new Uint8Array(1+i.length),this.registration[0]=1,this.registration.set(i,1)}acknowledge(e){if(e[0]!==1||this.binding)throw Error(`Invalid subscription acknowledgement`);let t=S([x.encode(`plain-graphql-ack-v1`),x.encode(`/graphql`),x.encode(this.client),r(this.registration)]),n=JSON.parse(new TextDecoder().decode(w(this.key,e.subarray(1),t)));if(!/^[0-9a-f]{64}$/.test(n.serverNonce))throw Error(`Invalid subscription server nonce`);this.binding=r(S([this.registration,e]))}aad(e,t){if(!this.binding)throw Error(`Subscription handshake incomplete`);let n=new Uint8Array(8);return new DataView(n.buffer).setBigUint64(0,t,!1),S([x.encode(`plain-graphql-frame-v1`),x.encode(`/graphql`),x.encode(this.client),this.binding,x.encode(e),n])}encode(e){if(this.sent===18446744073709551615n)throw Error(`Subscription sequence exhausted`);let t=C(this.key,x.encode(JSON.stringify(e)),this.aad(`client`,this.sent)),n=new Uint8Array(9+t.length);return n[0]=1,new DataView(n.buffer).setBigUint64(1,this.sent,!1),n.set(t,9),this.sent++,n}decode(e){if(e[0]!==1||e.length<49)throw Error(`Invalid subscription frame`);let t=new DataView(e.buffer,e.byteOffset,e.byteLength).getBigUint64(1,!1);if(t!==this.received||t===18446744073709551615n)throw Error(`Invalid subscription sequence`);let n=w(this.key,e.subarray(9),this.aad(`server`,t));return this.received++,JSON.parse(new TextDecoder().decode(n))}};function T(e){return n()?D(e):E(e)}function E(e){let t=new URL(`/graphql`,d()),n=i(),r=l();if(!r)throw Error(`Subscription session missing`);t.searchParams.set(`cid`,n);let a=c(t.toString(),u(),`plain-graphql-ws-v1`);a.binaryType=`arraybuffer`;let o=!0,s=!1,f,m,h,g=setTimeout(()=>v(Error(`Subscription handshake timeout`)),1e4),_={send(e){try{if(!o||!s||!f)throw Error(`Subscription connection unavailable`);a.send(f.encode(e))}catch(e){v(e)}},close(){o=!1,f=void 0,clearTimeout(g),clearTimeout(h),clearInterval(m),a.close()}};function v(t){o&&(_.close(),e.dropped(t))}return a.onopen=()=>{if(o){if(a.protocol!==`plain-graphql-ws-v1`){v(Error(`Subscription protocol mismatch`));return}try{f=new ee(n,p(r)),a.send(f.registration)}catch(e){v(e)}}},a.onmessage=t=>{if(o)try{if(!f||!(t.data instanceof ArrayBuffer))throw Error(`Encrypted subscription frame required`);if(!s)f.acknowledge(new Uint8Array(t.data)),s=!0,clearTimeout(g),e.opened(),m=setInterval(()=>{_.send({type:`ping`}),h=setTimeout(()=>v(Error(`Subscription heartbeat timeout`)),1e4)},2e4);else{let n=f.decode(new Uint8Array(t.data));typeof n==`object`&&n&&`type`in n&&n.type===`pong`?clearTimeout(h):e.message(n)}}catch(e){v(e)}},a.onerror=()=>v(Error(`Subscription connection error`)),a.onclose=()=>v(Error(`Subscription connection closed`)),_}function D(t){let n=!0,r,i=Promise.resolve(),a={send(t){if(!n||r===void 0)throw Error(`Native subscription unavailable`);let a=r;i=i.then(()=>n?e(`native_subscription_send`,{handle:a,message:JSON.stringify(t)}):void 0).catch(o)},close(){n=!1,r!==void 0&&e(`native_subscription_close`,{handle:r}).catch(console.error),r=void 0}};function o(e){n&&(a.close(),t.dropped(e))}return(async()=>{let i=await e(`native_subscription_open`);if(!n){await e(`native_subscription_close`,{handle:i});return}for(r=i,t.opened();n;){let r=await e(`native_subscription_read`,{handle:i});n&&t.message(JSON.parse(r))}})().catch(o),a}var O=new class{connection;operations=new Map;ids=new Map;sequence=0;pending=new Set;acknowledged=!1;active=!1;retry;failures=0;generation=0;consumers=new Map;available=[];attachments=new Set;connectionListeners=new Set;onConnectionChange(e){return this.connectionListeners.add(e),()=>{this.connectionListeners.delete(e)}}subscribe(e,t){let n=this.consumers.get(e)??new Set;this.consumers.set(e,n);let r=e=>t(e);return n.add(r),this.updateOperations(),()=>{n.delete(r)&&(n.size||this.consumers.delete(e),this.updateOperations())}}onAttached(e){let t=()=>e();return this.attachments.add(t),()=>{this.attachments.delete(t)}}ready(){for(let e of[...this.attachments])if(this.attachments.has(e))try{e()}catch(e){console.error(`GraphQL subscription snapshot failed`,e)}}deliver(e,t){let n=this.consumers.get(e.field);if(n){for(let r of[...n])if(n.has(r))try{let n=r(t);n&&n.catch(t=>console.error(`GraphQL subscription consumer failed`,e.field,t))}catch(t){console.error(`GraphQL subscription consumer failed`,e.field,t)}}}start(){this.active=!0,this.connect()}setOperations(e){this.available=e,this.updateOperations()}updateOperations(){let e=new Map(this.available.filter(e=>this.consumers.has(e.field)).map(e=>[e.field,e]));for(let t of this.operations.keys())if(!e.has(t))for(let[e,n]of this.ids)n===t&&(this.acknowledged&&this.connection?.send({id:e,type:`complete`}),this.ids.delete(e),this.pending.delete(e));for(let[t,n]of e)!this.operations.has(t)&&this.acknowledged&&this.send(n);this.operations=e}send(e){let t=`${this.generation}:${++this.sequence}`;this.ids.set(t,e.field),this.pending.add(t),this.connection?.send({type:`subscribe`,id:t,payload:{query:e.document}})}connect(){if(!this.active)return;let e=++this.generation;try{this.connection=T({opened:()=>{if(e===this.generation){this.acknowledged=!0;for(let e of this.connectionListeners)e(!0);this.failures=0,this.pending.clear(),this.ids.clear();for(let e of this.operations.values())this.send(e)}},message:t=>{e===this.generation&&this.receive(t)},dropped:t=>{e===this.generation&&this.dropped(t)}})}catch(e){this.dropped(e)}}receive(e){if(!e||typeof e!=`object`||!(`type`in e)||!(`id`in e)||typeof e.id!=`string`)throw Error(`Invalid subscription message`);let t=e,n=this.operations.get(this.ids.get(t.id)??``);if(n)if(t.type===`next`){if(!t.payload||Array.isArray(t.payload))throw Error(`Invalid subscription result`);t.payload.extensions?.subscriptionReady&&this.settled(t.id),t.payload.errors?.length&&console.error(`GraphQL subscription errors`,n.field,t.payload.errors),t.payload.data&&Object.hasOwn(t.payload.data,n.field)&&this.deliver(n,t.payload.data[n.field])}else if(t.type===`error`){if(!Array.isArray(t.payload))throw Error(`Invalid subscription error`);if(console.error(`GraphQL subscription failed`,n.field,t.payload),t.payload.some(e=>e.message===`resync_required`)){this.dropped(Error(`Subscription resynchronization required`),!0);return}this.ids.delete(t.id),this.settled(t.id)}else if(t.type===`complete`)this.ids.delete(t.id),this.settled(t.id);else throw Error(`Unknown subscription message`)}settled(e){this.pending.delete(e)&&this.pending.size===0&&this.ready()}dropped(e,t=!1){console.error(e),this.generation++,this.connection?.close(),this.acknowledged=!1;for(let e of this.connectionListeners)e(!1);this.connection=void 0,this.pending.clear(),this.ids.clear(),this.active&&(t||!n())&&(this.retry=setTimeout(()=>this.connect(),Math.min(5e3,++this.failures*1e3)))}stop(){this.active=!1;for(let e of this.connectionListeners)e(null);this.generation++,clearTimeout(this.retry),this.acknowledged=!1,this.connection?.close(),this.connection=void 0,this.pending.clear(),this.ids.clear()}},k=`
  fragment TagFragment on Tag {
    id
    name
    count
  }
`,A=`
  fragment TagSubFragment on Tag {
    id
    name
  }
`,j=`
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
    buildChannel
    permissions
    downloadsDir
    developerMode
    debug
  }
`,M=`
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
`,N=`
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
  ${A}
`,P=`
  fragment SmsConversationFragment on SmsConversation {
    id
    address
    snippet
    lastMessageAt
    messageCount
    read
  }
`,F=`
  fragment SmsConversationWithAddressesFragment on SmsConversation {
    id
    address
    addresses
    snippet
    lastMessageAt
    messageCount
    read
  }
`,I=`
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
  ${A}
`,L=`
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
  ${A}
`,R=`
  fragment FileFragment on File {
    path
    isDir
    createdAt
    updatedAt
    size
    childCount
    mediaId
  }
`,ne=`
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
  ${A}
`,re=`
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
  ${A}
`,ie=`
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
  ${A}
`,z=`
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
  ${A}
`,B=`
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
  ${A}
`,V=`
  fragment FeedFragment on Feed {
    id
    name
    url
    logo
    fetchContent
    createdAt
    updatedAt
  }
`,H=`
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
  ${A}
`,U=`
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
`,W=`
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
`,G=`
  fragment ClipboardItemFragment on ClipboardItem {
    id
    text
    source
    label
    sensitive
    createdAt
  }
`,K=`
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
`,q=`
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
`,J=`
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
`,Y=`
  fragment BookmarkGroupFragment on BookmarkGroup {
    id
    name
    collapsed
    sortOrder
    itemCount
    createdAt
    updatedAt
  }
`,X=`
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

`;function Z(e){return e instanceof f?e.status===403?`desktop_access_disabled`:e.message:`network_error`}function Q(e){if(e)return typeof e==`function`?e():e}function ae(e){let t=y(!1),n=y(),r=0;async function i(i,a=!1){if(e.enabled&&!e.enabled())return;let s=r;t.value=!0;try{let t=i??Q(e.variables),c=await o(typeof e.document==`function`?e.document():e.document,t,{fresh:a});if(s!==r)return;c.errors?.length?e.handle(c.data,c.errors[0].message):(n.value=c.data,e.handle(c.data,``))}catch(t){if(s!==r||t instanceof f&&t.cancelled)return;e.handle(void 0,Z(t))}finally{s===r&&(t.value=!1)}}i();let a=!0,s=b();return s&&(m(()=>{a=!1}),h(()=>{a=!0})),typeof e.variables==`function`&&_(e.variables,async()=>{await v(),a&&i()},{deep:!0}),typeof e.document==`function`&&_(e.document,async()=>{await v(),a&&i()}),e.enabled&&_(e.enabled,async(e,t)=>{await v(),a&&e&&!t&&i()}),s&&g(O.onAttached(()=>{a&&(r++,i(void 0,!0))})),{loading:t,result:n,refetch:i}}function oe(e){let t=y(!1),n=y(),r=0,i=0,a=0,s,c=0,l=!0,u;b()&&(h(()=>{l=!0}),m(()=>{l=!1}),g(O.onAttached(()=>{l&&u&&(c++,d(u.variables,{...u.options,latest:!0,force:!0}))})));async function d(l,d={}){let p=c;u={variables:l,options:d};let m=++r,h=d.latest?++i:void 0,g=l??Q(e.variables),_=g?{...g}:void 0,v={variables:_,requestId:m,meta:d.meta};d.latest?s=m:a++,t.value=!0;try{let t=typeof e.document==`function`?e.document():e.document,r=d.force?await o(t,_,{fresh:!0}):await o(t,_);if(p!==c||d.latest&&h!==i)return;r.errors?.length?e.handle(r.data,r.errors[0].message,v):(n.value=r.data,e.handle(r.data,``,v))}catch(t){if(p!==c||t instanceof f&&t.cancelled)return;(!d.latest||h===i)&&e.handle(void 0,Z(t),v)}finally{d.latest?s===m&&(s=void 0):a--,t.value=a>0||s!==void 0}}return{loading:t,result:n,fetch:d}}var se=`
  query ($target: String!) {
    chatItems(target: $target, query: "", offset: 0, limit: 200) {
      ...ChatItemFragment
    }
  }
  ${M}
`;`${M}`;var ce=`
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
`,le=`
  query {
    chatConversations {
      id
      chat { ...ChatItemFragment }
    }
  }
  ${M}
`,ue=`
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
`,de=`
  query ($type: DataType!, $keys: [String!]!) {
    tagRelations(type: $type, keys: $keys) {
      tagId
      key
    }
  }
`,fe=`
  query {
    chatChannels {
      ...ChatChannelFragment
    }
  }
${X}
`,pe=`
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
`,me=`
  query sms($offset: Int!, $limit: Int!, $query: String!) {
    sms(query: $query, offset: $offset, limit: $limit) {
      ...SmsFragment
    }
    smsCount(query: $query)
  }
  ${N}
`,he=`
  query {
    sims {
      id
      label
      number
      subscriptionId
    }
  }
`,ge=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationFragment
    }
    smsConversationCount(query: $query)
  }
  ${P}
`,_e=`
  query smsConversations($offset: Int!, $limit: Int!, $query: String!) {
    smsConversations(query: $query, offset: $offset, limit: $limit) {
      ...SmsConversationWithAddressesFragment
    }
    smsConversationCount(query: $query)
  }
  ${F}
`,ve=`
  query contacts($offset: Int!, $limit: Int!, $query: String!) {
    contacts(query: $query, offset: $offset, limit: $limit) {
      ...ContactFragment
    }
    contactCount(query: $query)
  }
  ${I}
`,ye=`
  query {
    contactSources {
      name
      type
    }
  }
`,be=`
  query calls($offset: Int!, $limit: Int!, $query: String!) {
    calls(query: $query, offset: $offset, limit: $limit) {
      ...CallFragment
    }
    callCount(query: $query)
  }
  ${L}
`,xe=`
  query images($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    images(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...ImageFragment
    }
    imageCount(query: $query)
  }
  ${ne}
`,Se=`
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
`,Ce=`
  query {
    scanProgress {
      indexed
      pending
      total
      state
      error
    }
  }
`,we=`
  query videos($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    videos(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...VideoFragment
    }
    videoCount(query: $query)
  }
  ${re}
`,Te=`
  query audios($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: audios(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...AudioFragment
    }
    total: audioCount(query: $query)
  }
  ${ie}
`,Ee=`
  query files($root: String!, $offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    files(root: $root, query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...FileFragment
    }
  }
  ${R}
`,De=`
  query recentFiles {
    recentFiles {
      ...FileFragment
    }
  }
  ${R}
`,Oe=`
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
    }
  }
`,ke=`
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
`,Ae=`
  query {
    sambaSettings {
      enabled
      username
      hasPassword
      shares {
        name
        sharePath
        auth
        readOnly
      }
      serviceName
      serviceActive
      serviceEnabled
    }
  }
`,je=`
  query {
    favoriteFolders {
      rootPath
      fullPath
      alias
    }
  }
`,Me=`
  query {
    app {
      ...AppFragment
    }
  }
  ${te}
`,Ne=`
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
  ${j}
`,Pe=`
  query tags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
  }
  ${k}
`,Fe=`
  query mediaBuckets($type: MediaDataType!) {
    mediaBuckets(type: $type) {
      id
      name
      itemCount
      topItemPaths
    }
  }
`,Ie=`
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
  ${A}
`,Le=`
  query note($id: ID!) {
    note(id: $id) {
      ...NoteFragment
    }
  }
  ${z}
`,Re=`
  query {
    feeds {
      ...FeedFragment
    }
  }
  ${V}
`,ze=`
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
  ${A}
`,Be=`
  query feedsTags($type: DataType!) {
    tags(type: $type) {
      ...TagFragment
    }
    feeds {
      ...FeedFragment
    }
  }
  ${V}
  ${k}
`,Ve=`
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
  ${k}
`,He=`
  query feedEntry($id: ID!) {
    feedEntry(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${V}
  ${H}
`,Ue=`
  query imageCount($query: String!) {
    total: imageCount(query: $query)
    trash: imageCount(query: "trash:true")
  }
`,We=`
  query audioCount($query: String!) {
    total: audioCount(query: $query)
    trash: audioCount(query: "trash:true")
  }
`,Ge=`
  query docs($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    items: docs(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...DocFragment
    }
    total: docCount(query: $query)
  }
  ${B}
`,Ke=`
  query docCount($query: String!) {
    total: docCount(query: $query)
    trash: docCount(query: "trash:true")
    extGroups: docExtGroups {
      ext
      count
    }
  }
`,qe=`
  query videoCount($query: String!) {
    total: videoCount(query: $query)
    trash: videoCount(query: "trash:true")
  }
`,Je=`
  query {
    total: packageCount(query: "")
    system: packageCount(query: "type:SYSTEM")
  }
`,Ye=`
  query {
    total: feedEntryCount(query: "")
    today: feedEntryCount(query: "today:true")
    feedEntryCounts {
      id
      count
    }
  }
`,Xe=`
  query {
    total: contactCount(query: "")
  }
`,Ze=`
  query {
    total: callCount(query: "")
    incoming: callCount(query: "type:1")
    outgoing: callCount(query: "type:2")
    missed: callCount(query: "type:3")
  }
`,Qe=`
  query {
    smsBoxCounts {
      total
      inbox
      sent
      drafts
    }
  }
`,$e=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationFragment
    }
  }
  ${P}
`,et=`
  query {
    archivedSmsConversations(query: "", offset: 0, limit: 200) {
      ...SmsConversationWithAddressesFragment
    }
  }
  ${F}
`,tt=`
  query {
    total: noteCount(query: "")
    trash: noteCount(query: "trash:true")
  }
`,nt=`
  query packages($offset: Int!, $limit: Int!, $query: String!, $sortBy: FileSortBy!) {
    packages(query: $query, offset: $offset, limit: $limit, sortBy: $sortBy) {
      ...PackageFragment
    }
    packageCount(query: $query)
  }
  ${U}
`,rt=`
  query packageStatuses($ids: [ID!]!) {
    packageStatuses(ids: $ids) {
      id
      exists
      updatedAt
    }
  }
`,it=`
  query {
    isScreenMirroring
    screenMirrorControlEnabled
    screenMirrorQuality {
      mode
      resolution
    }
  }
`,at=`
  query {
    screenMirrorVideoCodec {
      annexB
      keyFrame
    }
  }
`,ot=`
  query {
    screenMirrorControlEnabled
  }
`,st=`
  mutation {
    requestScreenMirrorKeyFrame
  }
`,ct=`
  query {
    notifications(query: "", offset: 0, limit: 200) {
      ...NotificationFragment
    }
  }
  ${W}
`,lt=`
  query clipboardItems($offset: Int!, $limit: Int!, $query: String!) {
    clipboardItems(query: $query, offset: $offset, limit: $limit) {
      ...ClipboardItemFragment
    }
    clipboardItemCount(query: $query)
  }
  ${G}
`,ut=`
  query {
    deviceInfo {
      ...DeviceInfoFragment
    }
    deviceStatus {
      ...DeviceStatusFragment
    }
  }
  ${K}
  ${q}
`,dt=`
  query {
    deviceStatus {
      batteryLevel
      charging
    }
  }
`,ft=`
  query AppLogs($offset: Int!, $limit: Int!) {
    appLogs(query: "", offset: $offset, limit: $limit)
  }
`,$=`
  query {
    appLogPath
  }
`,pt=`
  query {
    dbPath
  }
`,mt=`
  query uploadedChunks($fileId: String!) {
    uploadedChunks(fileId: $fileId)
  }
`,ht=`
  query mergeStatus($fileId: String!) {
    mergeStatus(fileId: $fileId) {
      status
      value
      mergedSize
      error
    }
  }
`,gt=`
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
`,_t=`
  query {
    prefs
  }
`,vt=`
  query {
    dbTables
  }
`,yt=`
  query DbTableRowCount($table: String!) {
    dbTableRowCount(table: $table)
  }
`,bt=`
  query DbTableRows($table: String!, $offset: Int!, $limit: Int!) {
    dbTableRows(table: $table, offset: $offset, limit: $limit)
  }
`,xt=`
  query DbTableInfo($table: String!) {
    dbTableInfo(table: $table) {
      idKey
    }
  }
`,St=`
  query DbTableColumns($table: String!) {
    dbTableColumns(table: $table) {
      name
      dataType
      notNull
      defaultValue
      primaryKey
    }
  }
`,Ct=`
  query {
    bookmarks {
      ...BookmarkFragment
    }
    bookmarkGroups {
      ...BookmarkGroupFragment
    }
  }
  ${J}
  ${Y}
`,wt=`
  query appUpdate {
    appUpdate { currentVersion latestVersion hasUpdate url releaseName publishedAt }
  }
`,Tt=`
  query NearbyDevices {
    nearbyDevices { id name ips bestIp port deviceType version platform lastSeen status discoveryMethods }
    pairingStates { deviceId generation mode phase error }
    mdnsState { scanning receiver }
  }
`;export{Ie as $,ke as A,H as At,Ee as B,St as C,j as Ct,vt as D,M as Dt,bt as E,X as Et,Ye as F,O as Ft,ae as G,Se as H,He as I,ht as J,it as K,Re as L,Ge as M,R as Mt,je as N,z as Nt,ut as O,G as Ot,ze as P,k as Pt,Le as Q,Be as R,pt as S,we as St,yt as T,Y as Tt,xe as U,Ue as V,oe as W,Tt as X,Oe as Y,tt as Z,se as _,me as _t,wt as a,gt as at,ye as b,mt as bt,We as c,st as ct,Ct as d,ot as dt,ct as et,Ve as f,at as ft,le as g,Qe as gt,fe as h,_e as ht,ft as i,ce as it,Ke as j,V as jt,dt as k,I as kt,Ne as l,Ae as lt,be as m,ge as mt,Me as n,rt as nt,$e as o,_t as ot,Ze as p,he as pt,Fe as q,$ as r,nt as rt,et as s,De as st,ue as t,Je as tt,Te as u,Ce as ut,lt as v,de as vt,xt as w,J as wt,ve as x,qe as xt,Xe as y,Pe as yt,pe as z};