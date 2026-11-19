import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{ct as n}from"./chunk-http-DvrUI4GH.js";import{L as r,i}from"./chunk-gql-client-C-nXPNNK.js";import{At as a,Bt as o,Ft as s,It as c,Lt as l,Mt as u,Nt as d,Pt as f,Rt as p,jt as m,zt as h}from"./chunk-query-CwqMCw2V.js";function ee(a,o=!0){let s=e(!1),c=[],l=[];async function u(e){s.value=!0;try{let r=await i(a.document,e,{dedupe:!1,...a.origin?{origin:!0}:{}});if(r.errors?.length){let e=r.errors[0].message;o&&t.emit(`toast`,e);let i=n.fromGraphql(r.errors[0]);for(let e of l)e(i);return}for(let e of c)e(r);return r}catch(e){if(e instanceof n&&e.canceled)return;let i=e instanceof n?e.message:r.NetworkError;o&&t.emit(`toast`,i);for(let t of l)t(e);return}finally{s.value=!1}}function d(e){return c.push(e),{off:()=>{let t=c.indexOf(e);t>=0&&c.splice(t,1)}}}function f(e){return l.push(e),{off:()=>{let t=l.indexOf(e);t>=0&&l.splice(t,1)}}}return{mutate:u,loading:s,onDone:d,onError:f}}async function te(e,t){return await e(t)!=null}var g=`
  mutation {
    clearAppLogs
  }
`,_=`
  mutation setPref($key: String!, $value: JSON!) {
    setPref(key: $key, value: $value)
  }
`,v=`
  mutation updateDeviceName($name: String!) {
    updateDeviceName(name: $name)
  }
`,y=`
  mutation forwardChatItems($id: ID!, $targets: [String!]!, $note: String) {
    forwardChatItems(id: $id, targets: $targets, note: $note) {
      ...ChatItemFragment
    }
  }
  ${f}
`,b=`
  mutation sendChatItems($targets: [String!]!, $content: ChatContentInput!) {
    sendChatItems(targets: $targets, content: $content) {
      ...ChatItemFragment
    }
  }
  ${f}
`,x=`
  mutation deleteChatItem($id: ID!) {
    deleteChatItem(id: $id)
  }
`,S=`
  mutation deleteChatItems($query: String!) {
    deleteChatItems(query: $query) {
      affectedCount
    }
  }
`,C=`
  mutation retryChatItem($id: ID!) {
    retryChatItem(id: $id) {
      ...ChatItemFragment
    }
  }
  ${f}
`,w=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${d}
`,T=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${d}
`,E=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,D=`
  mutation deletePeer($peerId: ID!) {
    deletePeer(peerId: $peerId)
  }
`,O=`
  mutation unpairPeer($peerId: ID!) {
    unpairPeer(peerId: $peerId)
  }
`,k=`
  mutation leaveChatChannel($id: ID!) {
    leaveChatChannel(id: $id)
  }
`,A=`
  mutation addChatChannelMember($id: ID!, $peerId: ID!) {
    addChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${d}
`,j=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${d}
`,M=`
  mutation acceptChatChannelInvite($id: ID!) {
    acceptChatChannelInvite(id: $id)
  }
`,N=`
  mutation declineChatChannelInvite($id: ID!) {
    declineChatChannelInvite(id: $id)
  }
`,P=`
  mutation createDir($path: String!) {
    createDir(path: $path) {
      ...FileFragment
    }
  }
  ${p}
`,F=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${p}
`,I=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,L=`
  mutation copyFileToDirectory($src: String!, $directory: String!, $name: String!) {
    copyFileToDirectory(src: $src, directory: $directory, name: $name)
  }
`,R=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,z=`
  mutation recordMediaPlay($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,V=`
  mutation selectAudio($mediaId: ID!) {
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,H=`
  mutation enqueueAndSelectAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${a}
`,U=`
  mutation updateAudioPlayMode($mode: MediaPlayMode!) {
    updateAudioPlayMode(mode: $mode)
  }
`,W=`
  mutation removeAudioFromQueue($mediaId: ID!) {
    removeAudioFromQueue(mediaId: $mediaId)
  }
`,G=`
  mutation addAudioToQueue($query: String!) {
    addAudioToQueue(query: $query)
  }
`,K=`
  mutation clearAudioQueue {
    clearAudioQueue
  }
`,q=`
  mutation reorderAudioQueue($mediaIds: [ID!]!) {
    reorderAudioQueue(mediaIds: $mediaIds)
  }
`,J=`
  mutation deleteMediaItems($type: MediaDataType!, $query: String!) {
    deleteMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Y=`
  mutation trashMediaItems($type: MediaDataType!, $query: String!) {
    trashMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,X=`
  mutation restoreMediaItems($type: MediaDataType!, $query: String!) {
    restoreMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Z=`
  mutation moveMediaItems($type: MediaDataType!, $query: String!, $destDir: String!) {
    moveMediaItems(type: $type, query: $query, destDir: $destDir) {
      affectedCount
    }
  }
`,Q=`
  mutation trashSms($query: String!) {
    trashSms(query: $query) {
      affectedCount
    }
  }
`,ne=`
  mutation restoreSms($query: String!) {
    restoreSms(query: $query) {
      affectedCount
    }
  }
`,re=`
  mutation deleteSms($query: String!) {
    deleteSms(query: $query) {
      affectedCount
    }
  }
`,ie=`
  mutation removeFromTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    removeFromTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ae=`
  mutation addToTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    addToTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,oe=`
  mutation updateTagRelations($type: DataType!, $keys: [String!]!, $addTagIds: [ID!]!, $removeTagIds: [ID!]!) {
    updateTagRelations(type: $type, keys: $keys, addTagIds: $addTagIds, removeTagIds: $removeTagIds)
  }
`,se=`
  mutation createTag($type: DataType!, $name: String!) {
    createTag(type: $type, name: $name) {
      ...TagFragment
    }
  }
  ${o}
`,ce=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${o}
`,le=`
  mutation deleteTag($id: ID!) {
    deleteTag(id: $id)
  }
`,ue=`
  mutation addFavoriteFolder($rootPath: String!, $fullPath: String!) {
    addFavoriteFolder(rootPath: $rootPath, fullPath: $fullPath) {
      rootPath
      fullPath
    }
  }
`,de=`
  mutation removeFavoriteFolder($fullPath: String!) {
    removeFavoriteFolder(fullPath: $fullPath) {
      rootPath
      fullPath
      alias
    }
  }
`,fe=`
  mutation setMountAlias($id: ID!, $alias: String!) {
    setMountAlias(id: $id, alias: $alias)
  }
`,pe=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,me=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${h}
`,he=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${h}
`,ge=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,xe=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,Se=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${l}
`,Ce=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,we=`
  mutation relaunchApp {
    relaunchApp
  }
`,Te=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Ee=`
  mutation requestAccess($feature: AccessFeature!) {
    requestAccess(feature: $feature)
  }
`,De=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,Oe=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${l}
`,ke=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,Ae=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${l}
  ${c}
`,je=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,Me=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Ne=`
  mutation sendSms($number: String!, $body: String!, $attachmentPaths: [String!]! = [], $subscriptionId: Int, $requestId: String!) {
    sendSms(number: $number, body: $body, attachmentPaths: $attachmentPaths, subscriptionId: $subscriptionId, requestId: $requestId) {
      requestId
      requiresConfirmation
    }
  }
`,Pe=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Fe=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Ie=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,Le=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Re=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,ze=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,Be=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Ve=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,He=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,Ue=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,We=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,Ge=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Ke=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,qe=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Je=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Ye=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Xe=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec) { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Ze=`
  mutation stopPomodoro {
    stopPomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Qe=`
  mutation pausePomodoro {
    pausePomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,$e=`mutation logout { logout }`,et=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,tt=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${m}
`,nt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${m}
`,rt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,it=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,at=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${u}
`,ot=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${u}
`,st=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,ct=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,lt=`
  mutation { importImageSearchModel }
`,ut=`
  mutation { enableImageSearch }
`,dt=`
  mutation { disableImageSearch }
`,ft=`
  mutation { cancelImageModelDownload }
`,pt=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,mt=`
  mutation { cancelImageIndex }
`,ht=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${s}
`,gt=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${s}
`,_t=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,yt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,$=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,bt=`
  mutation StartDiscovery {
    startDiscovery
  }
`,xt=`
  mutation StopDiscovery {
    stopDiscovery
  }
`,St=`
  mutation PairDevice($peerId: ID!, $input: PairPeerInput!) {
    pairPeer(peerId: $peerId, input: $input)
  }
`,Ct=`
  mutation cancelPairing($peerId: ID!) {
    cancelPairing(peerId: $peerId)
  }
`,wt=`
  mutation respondToPairing($peerId: ID!, $accepted: Boolean!) {
    respondToPairing(peerId: $peerId, accepted: $accepted)
  }
`,Tt=`
  mutation createDownload($input: DownloadInput!) {
    createDownload(input: $input) { id }
  }
`,Et=`
  mutation pauseDownload($id: ID!) { pauseDownload(id: $id) }
`,Dt=`
  mutation resumeDownload($id: ID!) { resumeDownload(id: $id) }
`,Ot=`
  mutation retryDownload($id: ID!) { retryDownload(id: $id) }
`,kt=`
  mutation pauseFileIndex($volumeId: ID) { pauseFileIndex(volumeId: $volumeId) }
`,At=`
  mutation resumeFileIndex($volumeId: ID) { resumeFileIndex(volumeId: $volumeId) }
`,jt=`
  mutation stopFileIndex($volumeId: ID) { stopFileIndex(volumeId: $volumeId) }
`,Mt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`;export{lt as $,ke as $t,S as A,At,_t as B,pe as Bt,se as C,Ee as Ct,be as D,ve as Dt,rt as E,X as Et,ye as F,B as Ft,le as G,pt as Gt,He as H,_ as Ht,vt as I,V as It,H as J,xt as Jt,dt as K,Xe as Kt,De as L,b as Lt,We as M,Ot as Mt,xe as N,te as Nt,E as O,ne as Ot,$ as P,Ke as Pt,Ce as Q,Ae as Qt,ct as R,Ne as Rt,me as S,Ue as St,st as T,wt as Tt,D as U,Ve as Ut,ge as V,fe as Vt,re as W,bt as Wt,y as X,Ze as Xt,Mt as Y,jt as Yt,et as Z,Be as Zt,ht as _,F as _n,de as _t,ue as a,O as an,qe as at,Se as b,I as bt,je as c,ot as cn,St as ct,Ct as d,v as dn,Qe as dt,Y as en,ee as et,g as f,Oe as fn,it as ft,w as g,oe as gn,j as gt,at as h,ce as hn,W as ht,A as i,Ie as in,Je as it,Ye as j,C as jt,x as k,Dt as kt,mt as l,T as ln,Et as lt,L as m,Ge as mn,we as mt,G as n,Q as nn,k as nt,ae as o,U as on,Z as ot,K as p,he as pn,z as pt,ut as q,Re as qt,tt as r,Fe as rn,$e as rt,Pe as s,nt as sn,Te as st,M as t,_e as tn,Le as tt,ft as u,gt as un,kt as ut,P as v,ie as vt,N as w,ze as wt,R as x,q as xt,Tt as y,yt,J as z,Me as zt};