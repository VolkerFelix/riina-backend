//! Tests for ML workout classification and multipliers
//! - Strength workouts: 1.5x multiplier
//! - HIIT workouts: 1.5x multiplier
//! - Cardio workouts: No multiplier (1.0x)

use reqwest::Client;

mod common;
use common::utils::{spawn_app, create_test_user_and_login, delete_test_user};
use common::workout_data_helpers::{create_workout_from_custom_hr_data, upload_workout_data_for_user, create_health_profile_for_user};
use common::admin_helpers::create_admin_user_and_login;

#[tokio::test]
async fn upload_strength_workout_with_1_5x_multiplier() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Real strength training heart rate data - characteristic pattern with intervals
    let hr_samples = vec![
        ("15:34:02".to_string(), 69), ("15:34:05".to_string(), 68), ("15:34:15".to_string(), 71),
        ("15:34:19".to_string(), 75), ("15:35:13".to_string(), 81), ("15:35:18".to_string(), 80),
        ("15:35:19".to_string(), 81), ("15:35:24".to_string(), 81), ("15:35:29".to_string(), 83),
        ("15:35:58".to_string(), 83), ("15:36:03".to_string(), 81), ("15:36:05".to_string(), 79),
        ("15:36:09".to_string(), 79), ("15:36:18".to_string(), 81), ("15:36:20".to_string(), 81),
        ("15:36:24".to_string(), 82), ("15:36:29".to_string(), 83), ("15:36:34".to_string(), 84),
        ("15:36:40".to_string(), 84), ("15:36:48".to_string(), 85), ("15:36:49".to_string(), 84),
        ("15:36:54".to_string(), 84), ("15:37:03".to_string(), 84), ("15:37:08".to_string(), 88),
        ("15:37:09".to_string(), 90), ("15:37:18".to_string(), 101), ("15:37:23".to_string(), 103),
        ("15:37:26".to_string(), 104), ("15:37:33".to_string(), 104), ("15:37:34".to_string(), 104),
        ("15:37:43".to_string(), 104), ("15:37:45".to_string(), 103), ("15:37:51".to_string(), 102),
        ("15:37:54".to_string(), 101), ("15:38:03".to_string(), 99), ("15:38:04".to_string(), 98),
        ("15:38:13".to_string(), 93), ("15:38:16".to_string(), 94), ("15:38:19".to_string(), 93),
        ("15:38:28".to_string(), 87), ("15:38:32".to_string(), 82), ("15:38:34".to_string(), 81),
        ("15:38:41".to_string(), 77), ("15:38:45".to_string(), 76), ("15:38:53".to_string(), 77),
        ("15:38:55".to_string(), 77), ("15:38:59".to_string(), 79), ("15:39:04".to_string(), 79),
        ("15:39:13".to_string(), 73), ("15:39:18".to_string(), 71), ("15:39:20".to_string(), 70),
        ("15:39:26".to_string(), 71), ("15:39:33".to_string(), 73), ("15:39:38".to_string(), 74),
        ("15:39:40".to_string(), 74), ("15:39:44".to_string(), 74), ("15:39:53".to_string(), 72),
        ("15:39:54".to_string(), 73), ("15:39:59".to_string(), 78), ("15:40:23".to_string(), 101),
        ("15:40:28".to_string(), 101), ("15:40:32".to_string(), 99), ("15:40:38".to_string(), 92),
        ("15:40:39".to_string(), 92), ("15:40:48".to_string(), 90), ("15:40:53".to_string(), 87),
        ("15:40:57".to_string(), 87), ("15:40:59".to_string(), 88), ("15:41:07".to_string(), 84),
        ("15:41:13".to_string(), 86), ("15:41:17".to_string(), 91), ("15:41:19".to_string(), 90),
        ("15:41:24".to_string(), 90), ("15:41:29".to_string(), 93), ("15:41:34".to_string(), 98),
        ("15:41:48".to_string(), 102), ("15:41:52".to_string(), 103), ("15:41:58".to_string(), 105),
        ("15:42:01".to_string(), 104), ("15:42:06".to_string(), 106), ("15:42:10".to_string(), 109),
        ("15:42:15".to_string(), 109), ("15:42:20".to_string(), 110), ("15:42:26".to_string(), 104),
        ("15:42:33".to_string(), 100), ("15:42:34".to_string(), 100), ("15:42:43".to_string(), 97),
        ("15:42:45".to_string(), 96), ("15:42:53".to_string(), 87), ("15:42:58".to_string(), 85),
        ("15:42:59".to_string(), 84), ("15:43:08".to_string(), 87), ("15:43:13".to_string(), 81),
        ("15:43:15".to_string(), 79), ("15:43:19".to_string(), 80), ("15:43:24".to_string(), 87),
        ("15:43:43".to_string(), 103), ("15:43:46".to_string(), 104), ("15:43:53".to_string(), 105),
        ("15:43:58".to_string(), 105), ("15:43:59".to_string(), 106), ("15:44:07".to_string(), 112),
        ("15:44:13".to_string(), 114), ("15:44:17".to_string(), 113), ("15:44:19".to_string(), 112),
        ("15:44:24".to_string(), 109), ("15:44:33".to_string(), 92), ("15:44:34".to_string(), 92),
        ("15:44:41".to_string(), 87), ("15:44:48".to_string(), 88), ("15:44:49".to_string(), 87),
        ("15:44:54".to_string(), 87), ("15:45:02".to_string(), 82), ("15:45:04".to_string(), 82),
        ("15:45:10".to_string(), 81), ("15:45:15".to_string(), 81), ("15:45:19".to_string(), 81),
        ("15:45:26".to_string(), 83), ("15:45:33".to_string(), 83), ("15:45:34".to_string(), 84),
        ("15:45:39".to_string(), 85), ("15:45:44".to_string(), 85), ("15:45:53".to_string(), 80),
        ("15:45:58".to_string(), 79), ("15:46:02".to_string(), 79), ("15:46:04".to_string(), 79),
        ("15:46:09".to_string(), 84), ("15:46:14".to_string(), 92), ("15:46:45".to_string(), 101),
        ("15:46:53".to_string(), 97), ("15:46:58".to_string(), 98), ("15:47:03".to_string(), 94),
        ("15:47:05".to_string(), 94), ("15:47:09".to_string(), 94), ("15:47:18".to_string(), 90),
        ("15:47:23".to_string(), 91), ("15:47:27".to_string(), 89), ("15:47:30".to_string(), 87),
        ("15:47:34".to_string(), 84), ("15:47:39".to_string(), 84), ("15:47:48".to_string(), 86),
        ("15:47:53".to_string(), 91), ("15:47:56".to_string(), 91), ("15:48:03".to_string(), 82),
        ("15:48:06".to_string(), 82), ("15:48:09".to_string(), 82), ("15:48:14".to_string(), 79),
        ("15:48:19".to_string(), 81), ("15:48:24".to_string(), 81), ("15:48:33".to_string(), 81),
        ("15:48:38".to_string(), 83), ("15:48:39".to_string(), 83), ("15:48:48".to_string(), 84),
        ("15:48:52".to_string(), 84), ("15:48:54".to_string(), 85), ("15:48:59".to_string(), 90),
        ("15:49:38".to_string(), 104), ("15:49:43".to_string(), 103), ("15:49:47".to_string(), 104),
        ("15:49:50".to_string(), 104), ("15:49:54".to_string(), 103), ("15:50:02".to_string(), 103),
        ("15:50:07".to_string(), 102), ("15:50:13".to_string(), 98), ("15:50:16".to_string(), 98),
        ("15:50:19".to_string(), 98), ("15:50:24".to_string(), 99), ("15:50:30".to_string(), 96),
        ("15:50:34".to_string(), 94), ("15:50:40".to_string(), 85), ("15:50:48".to_string(), 81),
        ("15:50:52".to_string(), 81), ("15:50:54".to_string(), 82), ("15:50:59".to_string(), 86),
        ("15:51:08".to_string(), 86), ("15:51:13".to_string(), 92), ("15:51:17".to_string(), 91),
        ("15:51:22".to_string(), 86), ("15:51:29".to_string(), 85), ("15:51:38".to_string(), 85),
        ("15:51:39".to_string(), 85), ("15:51:44".to_string(), 83), ("15:51:53".to_string(), 81),
        ("15:51:56".to_string(), 80), ("15:51:59".to_string(), 81), ("15:52:04".to_string(), 86),
        ("15:52:09".to_string(), 95), ("15:52:48".to_string(), 114), ("15:52:53".to_string(), 113),
        ("15:52:54".to_string(), 114), ("15:53:01".to_string(), 109), ("15:53:05".to_string(), 111),
        ("15:53:09".to_string(), 106), ("15:53:18".to_string(), 110), ("15:53:23".to_string(), 109),
        ("15:53:24".to_string(), 108), ("15:53:29".to_string(), 106), ("15:53:38".to_string(), 105),
        ("15:53:41".to_string(), 105), ("15:53:47".to_string(), 95), ("15:53:49".to_string(), 93),
        ("15:53:54".to_string(), 90), ("15:54:02".to_string(), 93), ("15:54:04".to_string(), 93),
        ("15:54:09".to_string(), 89), ("15:54:15".to_string(), 88), ("15:54:23".to_string(), 90),
        ("15:54:27".to_string(), 91), ("15:54:29".to_string(), 91), ("15:54:38".to_string(), 89),
        ("15:54:41".to_string(), 88), ("15:54:44".to_string(), 88), ("15:54:49".to_string(), 88),
        ("15:54:54".to_string(), 89), ("15:54:59".to_string(), 86), ("15:55:04".to_string(), 92),
        ("15:55:48".to_string(), 121), ("15:55:53".to_string(), 115), ("15:55:54".to_string(), 115),
        ("15:55:59".to_string(), 113), ("15:56:07".to_string(), 109), ("15:56:13".to_string(), 106),
        ("15:56:18".to_string(), 110), ("15:56:22".to_string(), 107), ("15:56:27".to_string(), 106),
        ("15:56:29".to_string(), 104), ("15:56:35".to_string(), 100), ("15:56:39".to_string(), 96),
        ("15:56:44".to_string(), 94), ("15:56:49".to_string(), 93), ("15:56:57".to_string(), 90),
        ("15:56:59".to_string(), 91), ("15:57:05".to_string(), 87), ("15:57:09".to_string(), 87),
        ("15:57:18".to_string(), 89), ("15:57:23".to_string(), 90), ("15:57:28".to_string(), 83),
        ("15:57:31".to_string(), 85), ("15:57:34".to_string(), 88), ("15:57:43".to_string(), 91),
        ("15:57:44".to_string(), 91), ("15:57:53".to_string(), 102), ("15:57:56".to_string(), 105),
        ("15:58:01".to_string(), 109), ("15:58:04".to_string(), 111), ("15:58:13".to_string(), 121),
        ("15:58:17".to_string(), 125), ("15:58:20".to_string(), 127), ("15:58:28".to_string(), 131),
        ("15:58:32".to_string(), 131), ("15:58:36".to_string(), 132), ("15:58:42".to_string(), 137),
        ("15:58:44".to_string(), 139), ("15:58:49".to_string(), 140), ("15:58:55".to_string(), 141),
        ("15:58:59".to_string(), 136), ("15:59:07".to_string(), 132), ("15:59:13".to_string(), 140),
        ("15:59:16".to_string(), 140), ("15:59:23".to_string(), 138), ("15:59:26".to_string(), 136),
        ("15:59:31".to_string(), 135), ("15:59:34".to_string(), 134), ("15:59:39".to_string(), 129),
        ("15:59:47".to_string(), 121), ("15:59:49".to_string(), 120), ("15:59:58".to_string(), 112),
        ("15:59:59".to_string(), 111), ("16:00:08".to_string(), 103), ("16:00:09".to_string(), 103),
        ("16:00:18".to_string(), 104), ("16:00:21".to_string(), 103), ("16:00:28".to_string(), 98),
        ("16:00:32".to_string(), 95), ("16:00:34".to_string(), 92), ("16:00:39".to_string(), 90),
        ("16:00:47".to_string(), 90), ("16:00:49".to_string(), 90), ("16:00:56".to_string(), 91),
        ("16:00:59".to_string(), 95), ("16:01:04".to_string(), 102), ("16:01:11".to_string(), 109),
        ("16:01:18".to_string(), 111), ("16:01:21".to_string(), 114), ("16:01:28".to_string(), 123),
        ("16:01:31".to_string(), 126), ("16:01:38".to_string(), 133), ("16:01:40".to_string(), 135),
        ("16:01:44".to_string(), 136), ("16:01:49".to_string(), 136), ("16:01:55".to_string(), 138),
        ("16:02:03".to_string(), 145), ("16:02:04".to_string(), 146), ("16:02:09".to_string(), 147),
        ("16:02:18".to_string(), 145), ("16:02:23".to_string(), 147), ("16:02:24".to_string(), 147),
        ("16:02:30".to_string(), 144), ("16:02:38".to_string(), 142), ("16:02:42".to_string(), 140),
        ("16:02:46".to_string(), 139), ("16:02:53".to_string(), 132), ("16:02:54".to_string(), 131),
        ("16:02:59".to_string(), 127), ("16:03:04".to_string(), 121), ("16:03:13".to_string(), 115),
        ("16:03:16".to_string(), 113), ("16:03:19".to_string(), 109), ("16:03:24".to_string(), 105),
        ("16:03:29".to_string(), 103), ("16:03:34".to_string(), 102), ("16:03:40".to_string(), 102),
        ("16:03:48".to_string(), 100), ("16:03:49".to_string(), 101), ("16:03:58".to_string(), 98),
        ("16:04:03".to_string(), 97), ("16:04:06".to_string(), 97), ("16:04:12".to_string(), 93),
        ("16:04:14".to_string(), 95), ("16:04:19".to_string(), 93), ("16:04:28".to_string(), 95),
        ("16:04:33".to_string(), 93), ("16:04:36".to_string(), 91), ("16:04:41".to_string(), 94),
        ("16:04:44".to_string(), 96), ("16:04:53".to_string(), 103), ("16:04:57".to_string(), 105),
        ("16:04:59".to_string(), 107), ("16:05:08".to_string(), 121), ("16:05:09".to_string(), 123),
        ("16:05:19".to_string(), 135), ("16:05:24".to_string(), 138), ("16:05:28".to_string(), 139),
        ("16:05:32".to_string(), 140), ("16:05:35".to_string(), 142), ("16:05:42".to_string(), 146),
        ("16:05:47".to_string(), 148), ("16:05:50".to_string(), 148), ("16:05:56".to_string(), 145),
        ("16:06:04".to_string(), 147), ("16:06:07".to_string(), 147), ("16:06:13".to_string(), 145),
        ("16:06:15".to_string(), 143), ("16:06:20".to_string(), 139), ("16:06:25".to_string(), 136),
        ("16:06:34".to_string(), 127), ("16:06:35".to_string(), 127), ("16:06:41".to_string(), 125),
        ("16:06:48".to_string(), 120), ("16:06:54".to_string(), 117), ("16:06:55".to_string(), 117),
        ("16:07:00".to_string(), 114), ("16:07:05".to_string(), 111), ("16:07:10".to_string(), 108),
        ("16:07:18".to_string(), 106), ("16:07:22".to_string(), 106), ("16:07:25".to_string(), 105),
        ("16:07:31".to_string(), 105), ("16:07:35".to_string(), 105), ("16:07:41".to_string(), 97),
        ("16:07:48".to_string(), 87), ("16:07:54".to_string(), 91), ("16:07:59".to_string(), 88),
        ("16:08:04".to_string(), 86), ("16:08:07".to_string(), 85), ("16:08:10".to_string(), 86),
        ("16:08:15".to_string(), 91), ("16:08:24".to_string(), 92), ("16:08:29".to_string(), 92),
        ("16:08:31".to_string(), 95), ("16:08:39".to_string(), 85), ("16:08:41".to_string(), 84),
        ("16:08:49".to_string(), 93), ("16:08:52".to_string(), 91), ("16:08:58".to_string(), 95),
        ("16:09:00".to_string(), 96), ("16:09:09".to_string(), 102), ("16:09:14".to_string(), 104),
        ("16:09:17".to_string(), 109), ("16:09:23".to_string(), 117), ("16:09:29".to_string(), 124),
        ("16:09:34".to_string(), 129), ("16:09:37".to_string(), 133), ("16:09:44".to_string(), 137),
        ("16:09:46".to_string(), 138), ("16:09:50".to_string(), 141), ("16:09:56".to_string(), 145),
        ("16:10:00".to_string(), 145), ("16:10:06".to_string(), 147), ("16:10:11".to_string(), 147),
        ("16:10:19".to_string(), 148), ("16:10:20".to_string(), 147), ("16:10:25".to_string(), 146),
        ("16:10:34".to_string(), 142), ("16:10:39".to_string(), 141), ("16:10:41".to_string(), 141),
        ("16:10:46".to_string(), 139), ("16:10:54".to_string(), 133), ("16:10:59".to_string(), 127),
        ("16:11:03".to_string(), 124), ("16:11:09".to_string(), 116), ("16:11:13".to_string(), 116),
        ("16:11:15".to_string(), 116), ("16:11:22".to_string(), 110), ("16:11:25".to_string(), 111),
        ("16:11:32".to_string(), 104), ("16:11:36".to_string(), 105), ("16:11:42".to_string(), 101),
        ("16:11:47".to_string(), 103), ("16:11:50".to_string(), 103), ("16:11:56".to_string(), 98),
        ("16:12:00".to_string(), 100), ("16:12:09".to_string(), 96), ("16:12:11".to_string(), 97),
        ("16:12:19".to_string(), 94), ("16:12:29".to_string(), 90), ("16:12:34".to_string(), 92),
        ("16:12:36".to_string(), 91), ("16:12:42".to_string(), 98), ("16:12:45".to_string(), 95),
        ("16:12:50".to_string(), 94), ("16:12:55".to_string(), 103), ("16:13:01".to_string(), 114),
        ("16:13:55".to_string(), 125), ("16:14:00".to_string(), 123), ("16:14:05".to_string(), 124),
        ("16:14:07".to_string(), 124), ("16:14:13".to_string(), 122), ("16:14:16".to_string(), 120),
        ("16:14:25".to_string(), 113), ("16:14:26".to_string(), 113), ("16:14:31".to_string(), 111),
        ("16:14:36".to_string(), 107), ("16:14:45".to_string(), 100), ("16:14:50".to_string(), 100),
        ("16:14:53".to_string(), 99), ("16:14:56".to_string(), 101), ("16:15:05".to_string(), 100),
        ("16:15:10".to_string(), 99), ("16:15:11".to_string(), 97), ("16:15:19".to_string(), 94),
        ("16:15:21".to_string(), 94), ("16:15:26".to_string(), 100), ("16:16:13".to_string(), 118),
        ("16:16:19".to_string(), 129), ("16:16:23".to_string(), 125), ("16:16:31".to_string(), 121),
        ("16:16:33".to_string(), 123), ("16:16:41".to_string(), 124), ("16:16:45".to_string(), 122),
        ("16:16:47".to_string(), 121), ("16:16:52".to_string(), 118), ("16:17:01".to_string(), 111),
        ("16:17:06".to_string(), 109), ("16:17:07".to_string(), 109), ("16:17:16".to_string(), 102),
        ("16:17:20".to_string(), 99), ("16:17:22".to_string(), 100), ("16:17:27".to_string(), 100),
        ("16:17:32".to_string(), 100), ("16:17:41".to_string(), 99), ("16:17:46".to_string(), 95),
        ("16:17:48".to_string(), 93), ("16:17:56".to_string(), 93), ("16:17:59".to_string(), 90),
        ("16:18:06".to_string(), 94), ("16:18:09".to_string(), 94), ("16:18:12".to_string(), 94),
        ("16:18:22".to_string(), 91), ("16:18:25".to_string(), 92), ("16:18:31".to_string(), 90),
        ("16:18:37".to_string(), 89), ("16:18:39".to_string(), 88), ("16:18:44".to_string(), 90),
        ("16:18:52".to_string(), 94), ("16:18:55".to_string(), 93), ("16:18:58".to_string(), 95),
        ("16:19:05".to_string(), 91), ("16:19:08".to_string(), 93), ("16:19:13".to_string(), 101),
        ("16:19:19".to_string(), 113), ("16:19:23".to_string(), 124), ("16:20:07".to_string(), 138),
        ("16:20:11".to_string(), 134), ("16:20:16".to_string(), 132), ("16:20:22".to_string(), 125),
        ("16:20:27".to_string(), 127), ("16:20:28".to_string(), 127), ("16:20:33".to_string(), 124),
        ("16:20:38".to_string(), 122), ("16:20:47".to_string(), 124), ("16:20:50".to_string(), 123),
        ("16:20:57".to_string(), 110), ("16:21:01".to_string(), 110), ("16:21:03".to_string(), 111),
        ("16:21:12".to_string(), 108), ("16:21:14".to_string(), 108), ("16:21:22".to_string(), 108),
        ("16:21:25".to_string(), 107), ("16:21:28".to_string(), 108), ("16:21:37".to_string(), 99),
        ("16:21:39".to_string(), 99), ("16:21:46".to_string(), 87), ("16:21:48".to_string(), 90),
        ("16:21:57".to_string(), 93), ("16:21:58".to_string(), 93), ("16:22:07".to_string(), 97),
        ("16:22:08".to_string(), 98), ("16:22:17".to_string(), 99), ("16:22:19".to_string(), 98),
        ("16:22:23".to_string(), 95), ("16:22:28".to_string(), 94), ("16:22:33".to_string(), 93),
        ("16:22:41".to_string(), 89), ("16:22:45".to_string(), 86), ("16:22:48".to_string(), 87),
        ("16:22:57".to_string(), 90), ("16:23:02".to_string(), 93), ("16:23:07".to_string(), 89),
        ("16:23:12".to_string(), 91), ("16:23:13".to_string(), 92), ("16:23:19".to_string(), 93),
        ("16:23:23".to_string(), 93), ("16:23:32".to_string(), 84), ("16:23:35".to_string(), 85),
        ("16:23:38".to_string(), 86), ("16:23:44".to_string(), 88), ("16:23:52".to_string(), 80),
        ("16:23:57".to_string(), 83), ("16:24:02".to_string(), 89), ("16:24:07".to_string(), 87),
        ("16:24:08".to_string(), 86), ("16:24:13".to_string(), 88), ("16:24:22".to_string(), 88),
        ("16:24:27".to_string(), 90), ("16:24:29".to_string(), 91), ("16:24:33".to_string(), 91),
        ("16:24:38".to_string(), 98), ("16:24:43".to_string(), 108), ("16:25:25".to_string(), 127),
        ("16:25:28".to_string(), 123), ("16:25:33".to_string(), 119), ("16:25:42".to_string(), 126),
        ("16:25:45".to_string(), 125), ("16:25:49".to_string(), 122), ("16:25:53".to_string(), 121),
        ("16:26:02".to_string(), 117), ("16:26:05".to_string(), 115), ("16:26:11".to_string(), 111),
        ("16:26:14".to_string(), 107), ("16:26:20".to_string(), 101), ("16:26:27".to_string(), 94),
        ("16:26:28".to_string(), 93), ("16:26:33".to_string(), 89), ("16:26:38".to_string(), 93),
        ("16:26:47".to_string(), 95), ("16:26:52".to_string(), 94), ("16:26:53".to_string(), 93),
        ("16:27:02".to_string(), 94), ("16:27:07".to_string(), 94), ("16:27:18".to_string(), 103),
        ("16:27:27".to_string(), 105), ("16:27:32".to_string(), 98), ("16:27:37".to_string(), 92),
        ("16:27:40".to_string(), 94), ("16:27:46".to_string(), 85), ("16:27:49".to_string(), 85),
        ("16:27:56".to_string(), 88), ("16:27:58".to_string(), 87), ("16:28:07".to_string(), 88),
        ("16:28:12".to_string(), 93), ("16:28:13".to_string(), 93), ("16:28:22".to_string(), 95),
        ("16:28:27".to_string(), 94), ("16:28:30".to_string(), 91), ("16:28:36".to_string(), 94),
        ("16:28:38".to_string(), 92), ("16:28:47".to_string(), 93), ("16:28:48".to_string(), 94),
        ("16:28:57".to_string(), 107), ("16:28:59".to_string(), 110), ("16:29:03".to_string(), 113),
        ("16:29:08".to_string(), 115), ("16:29:16".to_string(), 114), ("16:29:22".to_string(), 115),
        ("16:29:27".to_string(), 112), ("16:29:30".to_string(), 112), ("16:29:33".to_string(), 112),
        ("16:29:38".to_string(), 114), ("16:29:43".to_string(), 119), ("16:29:50".to_string(), 124),
        ("16:29:55".to_string(), 125), ("16:29:58".to_string(), 126), ("16:30:03".to_string(), 127),
        ("16:30:12".to_string(), 109), ("16:30:14".to_string(), 109), ("16:30:19".to_string(), 116),
        ("16:30:26".to_string(), 121), ("16:30:29".to_string(), 124), ("16:30:37".to_string(), 132),
        ("16:30:42".to_string(), 134), ("16:30:47".to_string(), 133), ("16:30:48".to_string(), 133),
        ("16:30:57".to_string(), 134), ("16:31:02".to_string(), 133), ("16:31:04".to_string(), 133),
        ("16:31:08".to_string(), 134), ("16:31:13".to_string(), 133), ("16:31:21".to_string(), 134),
        ("16:31:27".to_string(), 134), ("16:31:31".to_string(), 134), ("16:31:37".to_string(), 127),
        ("16:31:38".to_string(), 127), ("16:31:47".to_string(), 111), ("16:31:50".to_string(), 115),
        ("16:31:57".to_string(), 115), ("16:31:58".to_string(), 115), ("16:32:03".to_string(), 116),
        ("16:32:08".to_string(), 108), ("16:32:17".to_string(), 111), ("16:32:18".to_string(), 112),
        ("16:32:23".to_string(), 108), ("16:32:31".to_string(), 100), ("16:32:34".to_string(), 100),
        ("16:32:38".to_string(), 104), ("16:32:47".to_string(), 96), ("16:32:52".to_string(), 98),
        ("16:32:53".to_string(), 97), ("16:32:58".to_string(), 90), ("16:33:07".to_string(), 99),
        ("16:33:10".to_string(), 99), ("16:33:13".to_string(), 97), ("16:33:20".to_string(), 87),
        ("16:33:23".to_string(), 90), ("16:33:32".to_string(), 83), ("16:33:37".to_string(), 85),
        ("16:33:42".to_string(), 84), ("16:33:47".to_string(), 86), ("16:33:49".to_string(), 87),
        ("16:33:56".to_string(), 90), ("16:33:58".to_string(), 91), ("16:34:05".to_string(), 92),
        ("16:34:11".to_string(), 87), ("16:34:16".to_string(), 94), ("16:34:18".to_string(), 91),
        ("16:34:27".to_string(), 84), ("16:34:32".to_string(), 85), ("16:34:33".to_string(), 86),
        ("16:34:39".to_string(), 91), ("16:34:45".to_string(), 90), ("16:34:50".to_string(), 95),
        ("16:34:53".to_string(), 94), ("16:35:02".to_string(), 92), ("16:35:06".to_string(), 96),
        ("16:35:08".to_string(), 94), ("16:35:16".to_string(), 100), ("16:35:18".to_string(), 100),
        ("16:35:26".to_string(), 108), ("16:35:32".to_string(), 113), ("16:35:35".to_string(), 113),
        ("16:35:42".to_string(), 112), ("16:35:44".to_string(), 112), ("16:35:49".to_string(), 114),
        ("16:35:56".to_string(), 118), ("16:35:59".to_string(), 119), ("16:36:07".to_string(), 122),
        ("16:36:10".to_string(), 123), ("16:36:13".to_string(), 123), ("16:36:21".to_string(), 107),
        ("16:36:23".to_string(), 105), ("16:36:31".to_string(), 114), ("16:36:33".to_string(), 115),
        ("16:36:42".to_string(), 123), ("16:36:43".to_string(), 124), ("16:36:50".to_string(), 129),
        ("16:37:01".to_string(), 128), ("16:37:07".to_string(), 129), ("16:37:10".to_string(), 129),
        ("16:37:17".to_string(), 129), ("16:37:20".to_string(), 129), ("16:37:24".to_string(), 130),
        ("16:37:31".to_string(), 130), ("16:37:35".to_string(), 131), ("16:37:42".to_string(), 130),
        ("16:37:44".to_string(), 129), ("16:37:52".to_string(), 125), ("16:37:54".to_string(), 125),
        ("16:38:02".to_string(), 122), ("16:38:04".to_string(), 121), ("16:38:12".to_string(), 116),
        ("16:38:13".to_string(), 115), ("16:38:18".to_string(), 112), ("16:38:25".to_string(), 107),
        ("16:38:29".to_string(), 107), ("16:38:37".to_string(), 103), ("16:38:41".to_string(), 103),
        ("16:38:47".to_string(), 102), ("16:38:49".to_string(), 102), ("16:38:55".to_string(), 96),
        ("16:39:00".to_string(), 98), ("16:39:05".to_string(), 92), ("16:39:13".to_string(), 98),
        ("16:39:17".to_string(), 96), ("16:39:19".to_string(), 97), ("16:39:26".to_string(), 91),
        ("16:39:33".to_string(), 91), ("16:39:34".to_string(), 91), ("16:39:40".to_string(), 94),
        ("16:39:46".to_string(), 94), ("16:39:49".to_string(), 97), ("16:39:54".to_string(), 91),
        ("16:40:02".to_string(), 95), ("16:40:07".to_string(), 97), ("16:40:10".to_string(), 96),
        ("16:40:16".to_string(), 96), ("16:40:19".to_string(), 95), ("16:40:26".to_string(), 90),
        ("16:40:32".to_string(), 93), ("16:40:34".to_string(), 91), ("16:40:41".to_string(), 88),
        ("16:40:44".to_string(), 90), ("16:40:51".to_string(), 83), ("16:40:56".to_string(), 85),
        ("16:40:59".to_string(), 85), ("16:41:04".to_string(), 88), ("16:41:10".to_string(), 87),
        ("16:41:14".to_string(), 89), ("16:41:23".to_string(), 83), ("16:41:24".to_string(), 82),
        ("16:41:33".to_string(), 84), ("16:41:37".to_string(), 82), ("16:41:42".to_string(), 84),
        ("16:41:44".to_string(), 84), ("16:41:51".to_string(), 85), ("16:41:56".to_string(), 82),
        ("16:42:02".to_string(), 84), ("16:42:04".to_string(), 85), ("16:42:11".to_string(), 83),
        ("16:42:14".to_string(), 84), ("16:42:21".to_string(), 87), ("16:42:24".to_string(), 88),
        ("16:42:29".to_string(), 86), ("16:42:34".to_string(), 87), ("16:42:42".to_string(), 83),
        ("16:42:48".to_string(), 83), ("16:42:53".to_string(), 76), ("16:42:55".to_string(), 75),
        ("16:43:03".to_string(), 87), ("16:43:07".to_string(), 92), ("16:43:11".to_string(), 94),
        ("16:43:18".to_string(), 94), ("16:43:20".to_string(), 94), ("16:43:25".to_string(), 94),
        ("16:43:30".to_string(), 90), ("16:43:37".to_string(), 96), ("16:43:43".to_string(), 96),
        ("16:43:45".to_string(), 96), ("16:43:51".to_string(), 101), ("16:43:55".to_string(), 101),
        ("16:44:01".to_string(), 104), ("16:44:08".to_string(), 107), ("16:44:09".to_string(), 108),
        ("16:44:17".to_string(), 105), ("16:44:19".to_string(), 106), ("16:44:24".to_string(), 110),
        ("16:44:29".to_string(), 112), ("16:44:38".to_string(), 98), ("16:44:39".to_string(), 99),
        ("16:44:45".to_string(), 108), ("16:44:52".to_string(), 114), ("16:44:55".to_string(), 115),
        ("16:45:02".to_string(), 117), ("16:45:06".to_string(), 116), ("16:45:09".to_string(), 116),
        ("16:45:14".to_string(), 115), ("16:45:23".to_string(), 130), ("16:45:28".to_string(), 131),
        ("16:45:33".to_string(), 131), ("16:45:38".to_string(), 130), ("16:45:39".to_string(), 130),
        ("16:45:46".to_string(), 130), ("16:45:51".to_string(), 127), ("16:45:56".to_string(), 125),
        ("16:45:59".to_string(), 124), ("16:46:05".to_string(), 118), ("16:46:13".to_string(), 109),
        ("16:46:18".to_string(), 109), ("16:46:21".to_string(), 110), ("16:46:27".to_string(), 103),
        ("16:46:32".to_string(), 101), ("16:46:38".to_string(), 99), ("16:46:43".to_string(), 93),
        ("16:46:44".to_string(), 93), ("16:46:49".to_string(), 97), ("16:46:56".to_string(), 89),
        ("16:47:03".to_string(), 89), ("16:47:05".to_string(), 88), ("16:47:11".to_string(), 90),
        ("16:47:16".to_string(), 88), ("16:47:21".to_string(), 91), ("16:47:24".to_string(), 89),
        ("16:47:29".to_string(), 94), ("16:47:38".to_string(), 85), ("16:47:43".to_string(), 85),
        ("16:47:46".to_string(), 84), ("16:47:53".to_string(), 95), ("16:47:54".to_string(), 95),
        ("16:48:02".to_string(), 87), ("16:48:05".to_string(), 91), ("16:48:13".to_string(), 85),
        ("16:48:18".to_string(), 85), ("16:48:22".to_string(), 85), ("16:48:24".to_string(), 85),
        ("16:48:32".to_string(), 88), ("16:48:35".to_string(), 86), ("16:48:40".to_string(), 90),
        ("16:48:46".to_string(), 83), ("16:48:53".to_string(), 84), ("16:48:58".to_string(), 86),
        ("16:49:00".to_string(), 86), ("16:49:08".to_string(), 81), ("16:49:13".to_string(), 82),
        ("16:49:16".to_string(), 81), ("16:49:23".to_string(), 85), ("16:49:28".to_string(), 83),
        ("16:49:30".to_string(), 83), ("16:49:34".to_string(), 84), ("16:49:40".to_string(), 81),
        ("16:49:47".to_string(), 84), ("16:49:52".to_string(), 85), ("16:49:55".to_string(), 88),
        ("16:50:03".to_string(), 82), ("16:50:07".to_string(), 86), ("16:50:09".to_string(), 86),
    ];

    let mut workout_data = create_workout_from_custom_hr_data(hr_samples);

    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Strength workout should upload successfully: {:?}", response.err());

    let response_data = response.unwrap();

    // Extract the game stats from response
    let stamina_change = response_data["data"]["game_stats"]["stamina_change"].as_f64().unwrap();
    let strength_change = response_data["data"]["game_stats"]["strength_change"].as_f64().unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    println!("💪 Strength workout stats: stamina={}, strength={}", stamina_change, strength_change);

    // Verify the workout was processed
    assert!(stamina_change > 0.0, "Stamina should increase from workout");

    // Fetch workout details via admin API to verify ML classification
    let workout_detail_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin_user.token))
        .send()
        .await
        .expect("Failed to fetch workout details");

    assert!(workout_detail_response.status().is_success(), "Admin API should return workout details");

    let workout_detail: serde_json::Value = workout_detail_response
        .json()
        .await
        .expect("Failed to parse workout details");

    // Verify ML classification
    let ml_prediction = workout_detail["data"]["ml_prediction"].as_str();
    let ml_confidence = workout_detail["data"]["ml_confidence"].as_f64();
    println!("🤖 ML Classification: {:?} (confidence: {:?})", ml_prediction, ml_confidence);

    assert_eq!(
        ml_prediction,
        Some("strength"),
        "Workout should be classified as 'strength' by ML service"
    );

    // The multiplier should have been applied to the stats
    // stamina_change should be 1.5x the base score
    println!("✅ Workout correctly classified as strength and 1.5x multiplier applied");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn upload_cardio_workout_without_multiplier() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Real cardio workout heart rate data - characteristic sustained elevated HR pattern
    let hr_samples = vec![
        ("16:51:52".to_string(), 101), ("16:51:57".to_string(), 104), ("16:51:59".to_string(), 104),
        ("16:52:05".to_string(), 125), ("16:52:08".to_string(), 117), ("16:54:22".to_string(), 121),
        ("16:54:24".to_string(), 120), ("16:54:32".to_string(), 112), ("16:54:33".to_string(), 112),
        ("16:54:39".to_string(), 107), ("16:54:43".to_string(), 107), ("16:54:48".to_string(), 109),
        ("16:54:54".to_string(), 112), ("16:54:58".to_string(), 114), ("16:55:22".to_string(), 125),
        ("16:55:32".to_string(), 128), ("16:55:37".to_string(), 128), ("16:55:40".to_string(), 128),
        ("16:55:45".to_string(), 129), ("16:56:12".to_string(), 131), ("16:56:16".to_string(), 132),
        ("16:56:22".to_string(), 132), ("16:56:27".to_string(), 132), ("16:56:32".to_string(), 133),
        ("16:56:36".to_string(), 133), ("16:56:42".to_string(), 133), ("16:56:43".to_string(), 133),
        ("16:57:10".to_string(), 133), ("16:57:15".to_string(), 133), ("16:57:18".to_string(), 133),
        ("16:57:27".to_string(), 134), ("16:57:32".to_string(), 134), ("16:57:37".to_string(), 135),
        ("16:57:38".to_string(), 134), ("16:57:46".to_string(), 134), ("16:57:48".to_string(), 134),
        ("16:57:53".to_string(), 133), ("16:57:58".to_string(), 134), ("16:58:03".to_string(), 133),
        ("16:58:08".to_string(), 133), ("16:58:13".to_string(), 131), ("16:58:59".to_string(), 130),
        ("16:59:03".to_string(), 129), ("16:59:10".to_string(), 130), ("16:59:14".to_string(), 130),
        ("16:59:18".to_string(), 130), ("16:59:37".to_string(), 127), ("16:59:41".to_string(), 125),
        ("16:59:43".to_string(), 124), ("16:59:52".to_string(), 113), ("16:59:54".to_string(), 112),
        ("17:00:02".to_string(), 109), ("17:00:04".to_string(), 109), ("17:00:12".to_string(), 98),
        ("17:00:17".to_string(), 102), ("17:00:18".to_string(), 102), ("17:00:23".to_string(), 100),
        ("17:00:28".to_string(), 100), ("17:00:35".to_string(), 97), ("17:00:42".to_string(), 98),
        ("17:00:44".to_string(), 98), ("17:00:48".to_string(), 99), ("17:00:55".to_string(), 96),
        ("17:01:02".to_string(), 93), ("17:01:03".to_string(), 92), ("17:01:08".to_string(), 93),
        ("17:10:02".to_string(), 113), ("17:10:05".to_string(), 113), ("17:10:11".to_string(), 118),
        ("17:10:17".to_string(), 123), ("17:10:27".to_string(), 129), ("17:10:31".to_string(), 130),
        ("17:10:35".to_string(), 132), ("17:10:39".to_string(), 132), ("17:10:44".to_string(), 133),
        ("17:10:52".to_string(), 136), ("17:10:55".to_string(), 136), ("17:10:58".to_string(), 137),
        ("17:11:06".to_string(), 138), ("17:11:10".to_string(), 139), ("17:11:17".to_string(), 138),
        ("17:11:18".to_string(), 138), ("17:11:25".to_string(), 136), ("17:11:28".to_string(), 133),
        ("17:11:33".to_string(), 132), ("17:11:38".to_string(), 131), ("17:11:44".to_string(), 131),
        ("17:11:48".to_string(), 132), ("17:11:56".to_string(), 135), ("17:12:01".to_string(), 136),
        ("17:12:06".to_string(), 139), ("17:12:09".to_string(), 138), ("17:12:16".to_string(), 138),
        ("17:12:21".to_string(), 139), ("17:12:24".to_string(), 139), ("17:12:31".to_string(), 142),
        ("17:12:34".to_string(), 143), ("17:12:39".to_string(), 144), ("17:12:47".to_string(), 145),
        ("17:20:02".to_string(), 137), ("17:20:04".to_string(), 137), ("17:20:08".to_string(), 137),
        ("17:20:16".to_string(), 138), ("17:20:21".to_string(), 138), ("17:20:27".to_string(), 138),
        ("17:20:32".to_string(), 140), ("17:20:36".to_string(), 139), ("17:20:42".to_string(), 137),
        ("17:20:44".to_string(), 136), ("17:20:48".to_string(), 134), ("17:20:57".to_string(), 123),
        ("17:20:59".to_string(), 123), ("17:21:05".to_string(), 117), ("17:21:08".to_string(), 117),
        ("17:21:14".to_string(), 111), ("17:21:22".to_string(), 107), ("17:21:25".to_string(), 107),
        ("17:21:28".to_string(), 107), ("17:21:33".to_string(), 110), ("17:21:38".to_string(), 114),
        ("17:21:43".to_string(), 117), ("17:21:48".to_string(), 122), ("17:21:57".to_string(), 131),
        ("17:22:00".to_string(), 132), ("17:22:03".to_string(), 131), ("17:22:12".to_string(), 135),
        ("17:22:17".to_string(), 137), ("17:22:18".to_string(), 137), ("17:22:27".to_string(), 139),
        ("17:22:28".to_string(), 139), ("17:22:33".to_string(), 139), ("17:22:38".to_string(), 140),
        ("17:22:47".to_string(), 141), ("17:22:48".to_string(), 142), ("17:22:57".to_string(), 142),
        ("17:23:01".to_string(), 143), ("17:23:06".to_string(), 144), ("17:23:09".to_string(), 145),
        ("17:23:13".to_string(), 145), ("17:23:21".to_string(), 146), ("17:23:23".to_string(), 144),
        ("17:23:31".to_string(), 146), ("17:23:34".to_string(), 147), ("17:23:40".to_string(), 147),
        ("17:23:47".to_string(), 148), ("17:23:50".to_string(), 148), ("17:23:56".to_string(), 150),
        ("17:23:58".to_string(), 150), ("17:24:03".to_string(), 149), ("17:24:12".to_string(), 150),
        ("17:24:13".to_string(), 150), ("17:24:22".to_string(), 149), ("17:24:23".to_string(), 148),
        ("17:24:30".to_string(), 148), ("17:24:35".to_string(), 148), ("17:24:40".to_string(), 147),
        ("17:24:43".to_string(), 147), ("17:24:51".to_string(), 147), ("17:24:54".to_string(), 147),
        ("17:25:02".to_string(), 147), ("17:25:06".to_string(), 148), ("17:25:11".to_string(), 148),
        ("17:25:17".to_string(), 148), ("17:25:18".to_string(), 148), ("17:25:27".to_string(), 150),
        ("17:25:28".to_string(), 150), ("17:25:37".to_string(), 150), ("17:25:39".to_string(), 150),
        ("17:25:46".to_string(), 150), ("17:25:51".to_string(), 150), ("17:25:53".to_string(), 150),
        ("17:26:02".to_string(), 151), ("17:26:10".to_string(), 151), ("17:26:16".to_string(), 152),
        ("17:26:22".to_string(), 152), ("17:26:26".to_string(), 152), ("17:26:28".to_string(), 151),
        ("17:26:37".to_string(), 152), ("17:26:40".to_string(), 153), ("17:26:48".to_string(), 152),
        ("17:26:53".to_string(), 152), ("17:27:02".to_string(), 151), ("17:27:06".to_string(), 152),
        ("17:27:08".to_string(), 151), ("17:27:14".to_string(), 151), ("17:27:21".to_string(), 152),
        ("17:27:27".to_string(), 152), ("17:27:28".to_string(), 151), ("17:27:35".to_string(), 151),
        ("17:27:41".to_string(), 150), ("17:27:47".to_string(), 149), ("17:27:49".to_string(), 148),
        ("17:27:57".to_string(), 145), ("17:28:01".to_string(), 144), ("17:28:07".to_string(), 136),
        ("17:28:12".to_string(), 130), ("17:28:14".to_string(), 129), ("17:28:18".to_string(), 129),
        ("17:40:52".to_string(), 136), ("17:40:57".to_string(), 132), ("17:41:00".to_string(), 132),
        ("17:41:03".to_string(), 131), ("17:41:08".to_string(), 125), ("17:41:15".to_string(), 118),
        ("17:41:21".to_string(), 115), ("17:41:27".to_string(), 110), ("17:41:30".to_string(), 109),
        ("17:41:33".to_string(), 109), ("17:41:42".to_string(), 107), ("17:41:47".to_string(), 103),
        ("17:41:50".to_string(), 104), ("17:41:53".to_string(), 102), ("17:41:58".to_string(), 101),
        ("17:50:21".to_string(), 128), ("17:50:23".to_string(), 127), ("17:50:28".to_string(), 124),
        ("17:50:36".to_string(), 123), ("17:50:41".to_string(), 119), ("17:50:42".to_string(), 119),
        ("17:50:50".to_string(), 109), ("17:50:52".to_string(), 110), ("17:50:57".to_string(), 108),
        ("17:51:06".to_string(), 102), ("17:51:09".to_string(), 103), ("17:51:14".to_string(), 104),
        ("17:51:17".to_string(), 105), ("17:51:26".to_string(), 98), ("17:51:31".to_string(), 98),
        ("17:51:36".to_string(), 95), ("17:51:37".to_string(), 96),
    ];

    let mut workout_data = create_workout_from_custom_hr_data(hr_samples);

    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "Cardio workout should upload successfully: {:?}", response.err());

    let response_data = response.unwrap();

    // Extract the game stats from response
    let stamina_change = response_data["data"]["game_stats"]["stamina_change"].as_f64().unwrap();
    let strength_change = response_data["data"]["game_stats"]["strength_change"].as_f64().unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    println!("🏃 Cardio workout stats: stamina={}, strength={}", stamina_change, strength_change);

    // Verify the workout was processed
    assert!(stamina_change > 0.0, "Stamina should increase from workout");

    // Fetch workout details via admin API to verify ML classification
    let workout_detail_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin_user.token))
        .send()
        .await
        .expect("Failed to fetch workout details");

    assert!(workout_detail_response.status().is_success(), "Admin API should return workout details");

    let workout_detail: serde_json::Value = workout_detail_response
        .json()
        .await
        .expect("Failed to parse workout details");

    // Verify ML classification
    let ml_prediction = workout_detail["data"]["ml_prediction"].as_str();
    let ml_confidence = workout_detail["data"]["ml_confidence"].as_f64();
    println!("🤖 ML Classification: {:?} (confidence: {:?})", ml_prediction, ml_confidence);

    // Cardio workouts should NOT get the 1.5x multiplier (only strength workouts do)
    assert_eq!(
        ml_prediction,
        Some("cardio"),
        "Workout should be classified as 'cardio' by ML service"
    );

    // The important thing is cardio workouts do NOT get multiplied
    assert!(stamina_change > 0.0, "Workout should generate positive stamina");
    println!("✅ Cardio workout correctly classified without multiplier");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}

#[tokio::test]
async fn upload_hiit_workout_with_1_5x_multiplier() {
    let test_app = spawn_app().await;
    let client = Client::new();
    let test_user = create_test_user_and_login(&test_app.address).await;
    let admin_user = create_admin_user_and_login(&test_app.address).await;
    create_health_profile_for_user(&client, &test_app.address, &test_user).await.unwrap();

    // Real HIIT workout heart rate data - characteristic pattern with high-intensity intervals
    let hr_samples = vec![
        ("18:35:36".to_string(), 82), ("18:35:41".to_string(), 87), ("18:35:46".to_string(), 92),
        ("18:35:50".to_string(), 89), ("18:35:54".to_string(), 92), ("18:36:01".to_string(), 98),
        ("18:36:02".to_string(), 98), ("18:36:11".to_string(), 102), ("18:36:14".to_string(), 102),
        ("18:36:17".to_string(), 104), ("18:36:26".to_string(), 104), ("18:36:29".to_string(), 104),
        ("18:36:35".to_string(), 106), ("18:36:37".to_string(), 106), ("18:36:46".to_string(), 99),
        ("18:36:47".to_string(), 100), ("18:36:56".to_string(), 96), ("18:36:57".to_string(), 96),
        ("18:37:06".to_string(), 109), ("18:37:07".to_string(), 110), ("18:37:16".to_string(), 122),
        ("18:37:18".to_string(), 124), ("18:37:26".to_string(), 133), ("18:37:27".to_string(), 133),
        ("18:37:32".to_string(), 141), ("18:37:41".to_string(), 148), ("18:37:43".to_string(), 149),
        ("18:37:51".to_string(), 154), ("18:37:56".to_string(), 155), ("18:37:59".to_string(), 156),
        ("18:38:03".to_string(), 157), ("18:38:07".to_string(), 157), ("18:38:14".to_string(), 155),
        ("18:38:21".to_string(), 149), ("18:38:22".to_string(), 149), ("18:38:28".to_string(), 156),
        ("18:38:35".to_string(), 159), ("18:38:39".to_string(), 158), ("18:38:42".to_string(), 158),
        ("18:38:47".to_string(), 156), ("18:38:52".to_string(), 158), ("18:39:00".to_string(), 175),
        ("18:39:05".to_string(), 166), ("18:39:07".to_string(), 163), ("18:39:16".to_string(), 158),
        ("18:39:20".to_string(), 155), ("18:39:26".to_string(), 150), ("18:39:27".to_string(), 150),
        ("18:39:33".to_string(), 143), ("18:39:39".to_string(), 135), ("18:39:44".to_string(), 131),
        ("18:39:49".to_string(), 125), ("18:39:56".to_string(), 110), ("18:40:01".to_string(), 110),
        ("18:40:03".to_string(), 109), ("18:40:08".to_string(), 108), ("18:40:12".to_string(), 112),
        ("18:40:21".to_string(), 113), ("18:40:25".to_string(), 114), ("18:40:29".to_string(), 112),
        ("18:40:33".to_string(), 115), ("18:40:37".to_string(), 115), ("18:40:46".to_string(), 127),
        ("18:40:49".to_string(), 131), ("18:40:56".to_string(), 140), ("18:40:59".to_string(), 142),
        ("18:41:06".to_string(), 144), ("18:41:08".to_string(), 145), ("18:41:13".to_string(), 146),
        ("18:41:21".to_string(), 149), ("18:41:24".to_string(), 151), ("18:41:29".to_string(), 154),
        ("18:41:32".to_string(), 156), ("18:41:40".to_string(), 157), ("18:41:46".to_string(), 155),
        ("18:41:50".to_string(), 155), ("18:41:54".to_string(), 157), ("18:42:01".to_string(), 161),
        ("18:42:04".to_string(), 161), ("18:42:11".to_string(), 157), ("18:42:14".to_string(), 157),
        ("18:42:17".to_string(), 155), ("18:42:24".to_string(), 146), ("18:42:27".to_string(), 144),
        ("18:42:32".to_string(), 146), ("18:42:41".to_string(), 166), ("18:42:45".to_string(), 159),
        ("18:42:47".to_string(), 155), ("18:42:56".to_string(), 146), ("18:42:59".to_string(), 142),
        ("18:43:05".to_string(), 137), ("18:43:11".to_string(), 132), ("18:43:12".to_string(), 131),
        ("18:43:21".to_string(), 113), ("18:43:25".to_string(), 113), ("18:43:30".to_string(), 109),
        ("18:43:32".to_string(), 110), ("18:43:39".to_string(), 105), ("18:43:42".to_string(), 108),
        ("18:43:49".to_string(), 99), ("18:43:52".to_string(), 101), ("18:44:01".to_string(), 98),
        ("18:44:06".to_string(), 103), ("18:44:10".to_string(), 99), ("18:44:12".to_string(), 99),
        ("18:44:17".to_string(), 109), ("18:44:25".to_string(), 119), ("18:44:27".to_string(), 119),
        ("18:44:36".to_string(), 133), ("18:44:38".to_string(), 135), ("18:44:42".to_string(), 139),
        ("18:44:51".to_string(), 143), ("18:44:56".to_string(), 146), ("18:45:01".to_string(), 149),
        ("18:45:02".to_string(), 148), ("18:45:11".to_string(), 148), ("18:45:14".to_string(), 148),
        ("18:45:18".to_string(), 149), ("18:45:22".to_string(), 151), ("18:45:30".to_string(), 147),
        ("18:45:35".to_string(), 151), ("18:45:41".to_string(), 158), ("18:45:45".to_string(), 162),
        ("18:45:47".to_string(), 162), ("18:45:52".to_string(), 163), ("18:45:57".to_string(), 167),
        ("18:46:06".to_string(), 174), ("18:46:11".to_string(), 168), ("18:46:16".to_string(), 163),
        ("18:46:20".to_string(), 160), ("18:46:25".to_string(), 156), ("18:46:29".to_string(), 151),
        ("18:46:36".to_string(), 144), ("18:46:37".to_string(), 142), ("18:46:46".to_string(), 131),
        ("18:46:51".to_string(), 123), ("18:46:53".to_string(), 121), ("18:46:57".to_string(), 116),
        ("18:47:06".to_string(), 109), ("18:47:08".to_string(), 109), ("18:47:16".to_string(), 100),
        ("18:47:17".to_string(), 101), ("18:47:22".to_string(), 98), ("18:47:31".to_string(), 105),
        ("18:47:35".to_string(), 106), ("18:47:37".to_string(), 109), ("18:47:46".to_string(), 119),
        ("18:47:47".to_string(), 121), ("18:47:56".to_string(), 132), ("18:48:00".to_string(), 137),
        ("18:48:03".to_string(), 139), ("18:48:09".to_string(), 142), ("18:48:12".to_string(), 144),
        ("18:48:18".to_string(), 148), ("18:48:23".to_string(), 153), ("18:48:31".to_string(), 159),
        ("18:48:32".to_string(), 159), ("18:48:39".to_string(), 159), ("18:48:42".to_string(), 159),
        ("18:48:51".to_string(), 155), ("18:48:56".to_string(), 157), ("18:48:58".to_string(), 159),
        ("18:49:02".to_string(), 160), ("18:49:09".to_string(), 161), ("18:49:16".to_string(), 175),
        ("18:49:19".to_string(), 179), ("18:49:26".to_string(), 168), ("18:49:31".to_string(), 164),
        ("18:49:33".to_string(), 164), ("18:49:41".to_string(), 151), ("18:49:44".to_string(), 148),
        ("18:49:47".to_string(), 146), ("18:49:56".to_string(), 129), ("18:50:00".to_string(), 123),
        ("18:50:03".to_string(), 119), ("18:50:07".to_string(), 117), ("18:50:15".to_string(), 110),
        ("18:50:18".to_string(), 110), ("18:50:26".to_string(), 119), ("18:50:27".to_string(), 120),
        ("18:50:36".to_string(), 111), ("18:50:41".to_string(), 112), ("18:50:42".to_string(), 110),
        ("18:50:50".to_string(), 121), ("18:50:54".to_string(), 124), ("18:51:01".to_string(), 132),
        ("18:51:05".to_string(), 135), ("18:51:11".to_string(), 139), ("18:51:14".to_string(), 142),
        ("18:51:21".to_string(), 148), ("18:51:26".to_string(), 150), ("18:51:31".to_string(), 153),
        ("18:51:36".to_string(), 156), ("18:51:37".to_string(), 157), ("18:51:45".to_string(), 157),
        ("18:51:48".to_string(), 155), ("18:51:53".to_string(), 152), ("18:52:01".to_string(), 154),
        ("18:52:05".to_string(), 158), ("18:52:10".to_string(), 159), ("18:52:12".to_string(), 160),
        ("18:52:17".to_string(), 161), ("18:52:26".to_string(), 177), ("18:52:31".to_string(), 175),
        ("18:52:34".to_string(), 170), ("18:52:37".to_string(), 166), ("18:52:46".to_string(), 156),
        ("18:52:49".to_string(), 155), ("18:52:52".to_string(), 156), ("18:53:01".to_string(), 142),
        ("18:53:04".to_string(), 138), ("18:53:11".to_string(), 124), ("18:53:16".to_string(), 118),
        ("18:53:19".to_string(), 119), ("18:53:26".to_string(), 111), ("18:53:27".to_string(), 111),
        ("18:53:36".to_string(), 113), ("18:53:40".to_string(), 115), ("18:53:43".to_string(), 116),
        ("18:53:47".to_string(), 117), ("18:53:56".to_string(), 118), ("18:54:00".to_string(), 123),
        ("18:54:02".to_string(), 126), ("18:54:11".to_string(), 125), ("18:54:16".to_string(), 133),
        ("18:54:19".to_string(), 137), ("18:54:26".to_string(), 142), ("18:54:29".to_string(), 144),
        ("18:54:35".to_string(), 148), ("18:54:40".to_string(), 150), ("18:54:44".to_string(), 151),
        ("18:54:50".to_string(), 152), ("18:54:52".to_string(), 153), ("18:55:00".to_string(), 153),
        ("18:55:02".to_string(), 153), ("18:55:09".to_string(), 151), ("18:55:13".to_string(), 153),
        ("18:55:17".to_string(), 156), ("18:55:22".to_string(), 160), ("18:55:27".to_string(), 162),
        ("18:55:32".to_string(), 163), ("18:55:41".to_string(), 179), ("18:55:46".to_string(), 175),
        ("18:55:51".to_string(), 169), ("18:55:53".to_string(), 168), ("18:56:01".to_string(), 158),
        ("18:56:06".to_string(), 156), ("18:56:08".to_string(), 156), ("18:56:16".to_string(), 145),
        ("18:56:18".to_string(), 143), ("18:56:25".to_string(), 133), ("18:56:31".to_string(), 121),
        ("18:56:35".to_string(), 115), ("18:56:39".to_string(), 111), ("18:56:43".to_string(), 110),
        ("18:56:51".to_string(), 120), ("18:56:52".to_string(), 121), ("18:57:01".to_string(), 109),
        ("18:57:04".to_string(), 112), ("18:57:11".to_string(), 122), ("18:57:13".to_string(), 123),
        ("18:57:18".to_string(), 127), ("18:57:24".to_string(), 130), ("18:57:31".to_string(), 135),
        ("18:57:34".to_string(), 137), ("18:57:38".to_string(), 140), ("18:57:45".to_string(), 146),
        ("18:57:49".to_string(), 149), ("18:57:55".to_string(), 152), ("18:57:57".to_string(), 153),
        ("18:58:06".to_string(), 159), ("18:58:11".to_string(), 160), ("18:58:12".to_string(), 161),
        ("18:58:21".to_string(), 160), ("18:58:22".to_string(), 161), ("18:58:31".to_string(), 166),
        ("18:58:36".to_string(), 167), ("18:58:37".to_string(), 167), ("18:58:42".to_string(), 167),
        ("18:58:47".to_string(), 167), ("18:58:56".to_string(), 180), ("18:58:57".to_string(), 179),
        ("18:59:04".to_string(), 170), ("18:59:08".to_string(), 168), ("18:59:16".to_string(), 159),
        ("18:59:18".to_string(), 158), ("18:59:24".to_string(), 152), ("18:59:28".to_string(), 151),
        ("18:59:32".to_string(), 144), ("18:59:40".to_string(), 134), ("18:59:46".to_string(), 127),
        ("18:59:50".to_string(), 122), ("18:59:56".to_string(), 113), ("19:00:01".to_string(), 117),
        ("19:00:05".to_string(), 121), ("19:00:07".to_string(), 120), ("19:00:16".to_string(), 113),
        ("19:00:17".to_string(), 115), ("19:00:26".to_string(), 125), ("19:00:30".to_string(), 123),
        ("19:00:36".to_string(), 130), ("19:00:40".to_string(), 136), ("19:00:42".to_string(), 140),
        ("19:00:49".to_string(), 150), ("19:00:55".to_string(), 156), ("19:00:58".to_string(), 158),
        ("19:01:02".to_string(), 160), ("19:01:11".to_string(), 163), ("19:01:12".to_string(), 164),
        ("19:01:18".to_string(), 163), ("19:01:22".to_string(), 161), ("19:01:28".to_string(), 160),
        ("19:01:36".to_string(), 167), ("19:01:40".to_string(), 170), ("19:01:42".to_string(), 170),
        ("19:01:47".to_string(), 170), ("19:01:52".to_string(), 173), ("19:02:01".to_string(), 179),
        ("19:02:03".to_string(), 176), ("19:02:09".to_string(), 167), ("19:02:12".to_string(), 165),
        ("19:02:17".to_string(), 159), ("19:02:26".to_string(), 155), ("19:02:30".to_string(), 146),
        ("19:02:36".to_string(), 134), ("19:02:38".to_string(), 131), ("19:02:46".to_string(), 117),
        ("19:02:50".to_string(), 113), ("19:02:56".to_string(), 108), ("19:02:57".to_string(), 108),
        ("19:03:02".to_string(), 106), ("19:03:11".to_string(), 105),
    ];

    let mut workout_data = create_workout_from_custom_hr_data(hr_samples);

    let response = upload_workout_data_for_user(&client, &test_app.address, &test_user.token, &mut workout_data).await;
    assert!(response.is_ok(), "HIIT workout should upload successfully: {:?}", response.err());

    let response_data = response.unwrap();

    // Extract the game stats from response
    let stamina_change = response_data["data"]["game_stats"]["stamina_change"].as_f64().unwrap();
    let strength_change = response_data["data"]["game_stats"]["strength_change"].as_f64().unwrap();
    let sync_id = response_data["data"]["sync_id"].as_str().unwrap();

    println!("🔥 HIIT workout stats: stamina={}, strength={}", stamina_change, strength_change);

    // Verify the workout was processed
    assert!(stamina_change > 0.0, "Stamina should increase from workout");

    // Fetch workout details via admin API to verify ML classification
    let workout_detail_response = client
        .get(&format!("{}/admin/workouts/{}", &test_app.address, sync_id))
        .header("Authorization", format!("Bearer {}", admin_user.token))
        .send()
        .await
        .expect("Failed to fetch workout details");

    assert!(workout_detail_response.status().is_success(), "Admin API should return workout details");

    let workout_detail: serde_json::Value = workout_detail_response
        .json()
        .await
        .expect("Failed to parse workout details");

    // Verify ML classification
    let ml_prediction = workout_detail["data"]["ml_prediction"].as_str();
    let ml_confidence = workout_detail["data"]["ml_confidence"].as_f64();
    println!("🤖 ML Classification: {:?} (confidence: {:?})", ml_prediction, ml_confidence);

    assert_eq!(
        ml_prediction,
        Some("hiit"),
        "Workout should be classified as 'hiit' by ML service"
    );

    // The multiplier should have been applied to the stats
    // stamina_change should be 1.5x the base score
    println!("✅ Workout correctly classified as HIIT and 1.5x multiplier applied");

    // Cleanup
    delete_test_user(&test_app.address, &admin_user.token, test_user.user_id).await;
    delete_test_user(&test_app.address, &admin_user.token, admin_user.user_id).await;
}
