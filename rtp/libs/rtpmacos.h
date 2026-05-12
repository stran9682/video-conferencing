#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef void (*ReleaseCallback)(void*);

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

extern void swift_receive_video(void *context, const uint8_t *path);

extern void swift_release_pointer(void *context);

void swift_download(const uint8_t *tag,
                    uintptr_t tag_length,
                    const uint8_t *endpoint,
                    uintptr_t endpoint_length,
                    void *context);

void swift_upload(const uint8_t *file_path,
                  uintptr_t file_path_len,
                  const uint8_t *endpoint_id,
                  uintptr_t endpoint_id_length);

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
