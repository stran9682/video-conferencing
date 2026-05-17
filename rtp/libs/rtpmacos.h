#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct UploadManager UploadManager;

typedef void (*ReleaseCallback)(void*);

typedef void (*UpdateListCallback)(void *context, const uint8_t *ptr, uintptr_t count);

extern void swift_receive_sample(void *context, const uint8_t *audioData, uintptr_t length);

bool rust_get_address(int8_t *ptr);

void rust_set_video_callback(void *context);

void rust_set_audio_manger_context(void *context);

void rust_set_h264_args(const uint8_t *pps,
                        uintptr_t pps_length,
                        const uint8_t *sps,
                        uintptr_t sps_length);

void rust_set_opus_args(double sample_rate, uint32_t channels);

void rust_run_network_runtime(const uint8_t *endpoint_str, uintptr_t endpoint_str_length);

bool rust_send_audio_sample(const uint8_t *data, uintptr_t len, uint32_t timestamp);

bool rust_send_frame(const uint8_t *data,
                     uintptr_t len,
                     void *context,
                     ReleaseCallback release_callback,
                     uint32_t timestamp);

extern void swift_receive_frame(void *context, void *frameData, uintptr_t frameDataLength);

struct UploadManager *rust_setup_docs(void);

void rust_deallocate_uploadmanager(struct UploadManager *upload_manager_ptr);

bool rust_upload(struct UploadManager *upload_manager_ptr,
                 const uint8_t *file_path,
                 uintptr_t file_path_len,
                 const uint8_t *endpoint_id,
                 uintptr_t endpoint_id_length);

bool rust_change_permissions(struct UploadManager *upload_manager_ptr,
                             const uint8_t *list_ptr,
                             uintptr_t ptr_length);

void rust_get_shared_videos(struct UploadManager *upload_manager_ptr,
                            void *context,
                            UpdateListCallback update_list_callback);

bool rust_get_doc_ticket(struct UploadManager *upload_manager_ptr,
                         const uint8_t *namespace_id_ptr,
                         uintptr_t ptr_length,
                         int8_t *buffer);

extern void swift_receive_video(void *context, const uint8_t *path);

extern void swift_release_pointer(void *context);

void swift_download(const uint8_t *tag,
                    uintptr_t tag_length,
                    const uint8_t *endpoint,
                    uintptr_t endpoint_length,
                    void *context);

extern double swift_send_cmclocktime(void);

extern void *swift_receive_pps_sps(void *context,
                                   const uint8_t *pps,
                                   uintptr_t pps_length,
                                   const uint8_t *sps,
                                   uintptr_t sps_length,
                                   uint32_t ssrc);

extern void *swift_receive_audio_config(void *audio_manager_context,
                                        double sample_rate,
                                        uint32_t channels,
                                        uint32_t ssrc);

extern void swift_remove_audio_peer(void *audio_manager_context,
                                    uint32_t ssrc,
                                    void *participant_context);

extern void swift_remove_video_peer(uint32_t ssrc, void *video_manager_context, void *peer_context);
