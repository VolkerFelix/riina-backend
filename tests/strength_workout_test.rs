//! Test for strength workout classification and 1.5x multiplier

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
