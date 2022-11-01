# Postgres (for Mac)

## Install

```sh
brew install postgresql
...

# 確認
psql --version
```

## 起動, 停止

```sh
# 起動
brew services start postgresql
# 停止
brew services stop postgresql
```

## ログイン, 接続するユーザーとパスワードの設定, ログアウト

```sql
--  ログイン
psql database_name

--  接続するユーザーとパスワードの設定
# create user user_name password 'password';

--  ログアウト
# \q
```
